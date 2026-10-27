use aidoku::{
	Chapter, ContentRating, Manga, MangaPageResult, MangaStatus, Result, Viewer,
	alloc::{String, Vec},
	imports::{
		html::{Document, Element},
		std::parse_date,
	},
	prelude::*,
};
use base64::{
	Engine, alphabet,
	engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
};

use crate::helpers::{BASE_URL, chapter_key, manga_key, path_segments};

/// The site's base64 isn't always padded.
const BASE64: GeneralPurpose = GeneralPurpose::new(
	&alphabet::STANDARD,
	GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

pub fn has_next_page(html: &Document) -> bool {
	html.select_first("ul.pagination a[rel=next]").is_some()
}

/// Series cards from the directory, search results and the home page.
pub fn parse_manga_cards(html: &Document) -> MangaPageResult {
	MangaPageResult {
		entries: html
			.select("#article-div div._135yj")
			.map(|cards| cards.filter_map(|card| parse_manga_card(&card)).collect())
			.unwrap_or_default(),
		has_next_page: has_next_page(html),
	}
}

pub fn parse_manga_card(card: &Element) -> Option<Manga> {
	// Chapter cards on the home page link to the reader, so they're skipped here.
	let key = manga_key(&card.select_first("a[href]")?.attr("abs:href")?)?;
	let title = card.select_first("._2NNxg")?.text()?;
	Some(Manga {
		url: Some(format!("{BASE_URL}{key}")),
		key,
		title,
		cover: card.select_first("img").and_then(|img| image_url(&img)),
		..Default::default()
	})
}

/// Images are lazy loaded: `src` is a placeholder until `data-src` is copied over.
pub fn image_url(img: &Element) -> Option<String> {
	img.attr("abs:data-src")
		.filter(|src| !src.is_empty())
		.or_else(|| img.attr("abs:src"))
}

/// The rows of `/recientes`, or of the "Actualizado" section of the home page.
pub fn parse_latest_updates(html: &Document) -> Vec<(Manga, Option<Chapter>)> {
	html.select("div.NRoaY")
		.map(|rows| rows.filter_map(|row| parse_latest_update(&row)).collect())
		.unwrap_or_default()
}

fn parse_latest_update(row: &Element) -> Option<(Manga, Option<Chapter>)> {
	let link = row.select_first("a[data-test=latest-update-name]")?;
	let key = manga_key(&link.attr("abs:href")?)?;
	let manga = Manga {
		url: Some(format!("{BASE_URL}{key}")),
		key,
		title: link.text()?,
		cover: row.select_first("img").and_then(|img| image_url(&img)),
		..Default::default()
	};

	let chapter = row.select_first(".e6dEC a[href]").and_then(|link| {
		let url = link.attr("abs:href")?;
		let (chapter_number, title) = parse_chapter_title(&link.text().unwrap_or_default());
		Some(Chapter {
			key: chapter_key(&url)?,
			title,
			chapter_number,
			date_uploaded: row
				.select_first(".timeago[datetime]")
				.and_then(|date| parse_site_date(&date.attr("datetime")?)),
			url: Some(url),
			..Default::default()
		})
	});

	Some((manga, chapter))
}

/// Dates look like `2026-10-27 19:29:47` and are in UTC.
fn parse_site_date(date: &str) -> Option<i64> {
	parse_date(date, "yyyy-MM-dd HH:mm:ss")
}

/// Splits `Capítulo 243: Aquella Chica` into the number and the rest of the title.
fn parse_chapter_title(text: &str) -> (Option<f32>, Option<String>) {
	let text = text.trim();
	let Some(rest) = text.strip_prefix("Capítulo") else {
		return (None, Some(text.into()).filter(|t: &String| !t.is_empty()));
	};
	let rest = rest.trim_start();
	let (number, title) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
	let number = number.trim_end_matches(':');
	let title = title.trim_matches(|c: char| c.is_whitespace() || matches!(c, ':' | '-' | '—'));
	(
		number.parse().ok(),
		Some(title.into()).filter(|t: &String| !t.is_empty()),
	)
}

/// Fills in a series from its page.
pub fn parse_manga_details(html: &Document, manga: &mut Manga) {
	if let Some(title) = html.select_first("h1").and_then(|h1| h1.text()) {
		manga.title = title;
	}
	if let Some(cover) = html
		.select_first(".portada img")
		.and_then(|img| img.attr("abs:src"))
	{
		manga.cover = Some(cover);
	}
	manga.description = html
		.select_first("#sinopsis")
		.and_then(|sinopsis| sinopsis.text())
		// Drop the "Sinopsis" heading.
		.map(|text| text.trim_start_matches("Sinopsis").trim().into())
		.filter(|text: &String| !text.is_empty());

	// "Ranking: #9 Autor: Eiichirō Oda Fecha: 1997 Estado: En desarrollo"
	let info = html
		.select_first("#info-i")
		.and_then(|info| info.text())
		.unwrap_or_default();
	manga.authors = info
		.split_once("Autor:")
		.map(|(_, rest)| rest.split("Fecha:").next().unwrap_or_default())
		.map(|authors| {
			authors
				.split(',')
				.map(str::trim)
				.filter(|author| !author.is_empty())
				.map(String::from)
				.collect::<Vec<_>>()
		})
		.filter(|authors| !authors.is_empty());

	manga.status = match info.split_once("Estado:").map(|(_, status)| status.trim()) {
		Some(status) if status.starts_with("En desarrollo") => MangaStatus::Ongoing,
		Some(status) if status.starts_with("Finalizado") => MangaStatus::Completed,
		_ => MangaStatus::Unknown,
	};

	let tags: Vec<String> = html
		.select("#categ a")
		.map(|tags| tags.filter_map(|tag| tag.text()).collect())
		.unwrap_or_default();

	manga.content_rating = if html.select_first("#categ .adulto").is_some() {
		ContentRating::NSFW
	} else if tags.iter().any(|tag| tag == "Ecchi") {
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	};

	manga.viewer = match path_segments(&manga.key).first() {
		Some(&"manhwa" | &"manhua") => Viewer::Webtoon,
		_ if tags.iter().any(|tag| tag == "Webtoon") => Viewer::Webtoon,
		Some(&"manga" | &"oneshot") => Viewer::RightToLeft,
		_ => Viewer::Unknown,
	};

	manga.tags = Some(tags).filter(|tags| !tags.is_empty());
	manga.url = Some(format!("{BASE_URL}{}", manga.key));
}

/// Every chapter is listed on the series page, newest first.
pub fn parse_chapters(html: &Document) -> Vec<Chapter> {
	html.select("#c_list a[href]")
		.map(|links| {
			links
				.filter_map(|link| {
					let url = link.attr("abs:href")?;
					let info = link.select_first("span.timeago");
					let (title_number, title) = parse_chapter_title(
						&link
							.select_first("h3")
							.and_then(|h3| h3.text())
							.unwrap_or_default(),
					);
					Some(Chapter {
						key: chapter_key(&url)?,
						title,
						chapter_number: info
							.as_ref()
							.and_then(|info| info.attr("data-num")?.parse().ok())
							.or(title_number),
						date_uploaded: info
							.as_ref()
							.and_then(|info| parse_site_date(&info.attr("datetime")?)),
						// The user who uploaded the chapter, usually the scan group.
						scanlators: info
							.as_ref()
							.and_then(|info| info.attr("data-user"))
							.filter(|user| !user.is_empty())
							.map(|user| [user].into()),
						url: Some(url),
						..Default::default()
					})
				})
				.collect()
		})
		.unwrap_or_default()
}

/// The reader page has `var unicap = '<base64>'`, which decodes to
/// `<image folder url>||["001.webp?up=…","002.webp?up=…"]||`.
pub fn parse_page_urls(html: &Document) -> Result<Vec<String>> {
	let script = html
		.select("script")
		.and_then(|scripts| {
			scripts
				.filter_map(|script| script.html())
				.find(|data| data.contains("unicap"))
		})
		.ok_or_else(|| error!("No se encontraron las páginas del capítulo"))?;

	let encoded = script
		.split_once("unicap")
		.and_then(|(_, rest)| rest.split('\'').nth(1))
		.ok_or_else(|| error!("No se encontraron las páginas del capítulo"))?;
	let decoded = BASE64
		.decode(encoded)
		.ok()
		.and_then(|bytes| String::from_utf8(bytes).ok())
		.ok_or_else(|| error!("No se pudieron leer las páginas del capítulo"))?;

	let mut parts = decoded.split("||");
	let folder = parts.next().unwrap_or_default();
	let files: Vec<String> = parts
		.next()
		.and_then(|files| serde_json::from_str(files).ok())
		.ok_or_else(|| error!("No se pudieron leer las páginas del capítulo"))?;

	Ok(files.iter().map(|file| format!("{folder}{file}")).collect())
}
