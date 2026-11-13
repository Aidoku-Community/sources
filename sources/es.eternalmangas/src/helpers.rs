use crate::{URL, markdown, models};
use aidoku::{
	Chapter, ContentRating, Manga, MangaPageResult, MangaStatus, Page, PageContent, Result, Viewer,
	alloc::{String, Vec, vec},
	imports::{
		html::{Document, Element},
		net::{Request, Response},
		std::current_date,
	},
	prelude::*,
};

pub fn request(url: &str) -> Result<Request> {
	Ok(Request::get(url)?
		.header("Accept", "text/html")
		.header("Referer", &format!("{URL}/"))
		.timeout(30.))
}

pub fn check_response(response: Response, url: &str) -> Result<Response> {
	let status = response.status_code();
	if !(200..300).contains(&status) {
		let content_type = response.get_header("Content-Type");
		let prefix = response
			.get_string()
			.ok()
			.map(|body| body.chars().take(240).collect::<String>());
		println!(
			"URL: {url}; HTTP {status}; Content-Type: {content_type:?}; body prefix: {prefix:?}"
		);
		bail!("HTTP {status}");
	}
	Ok(response)
}

pub fn get_html(url: &str) -> Result<Document> {
	Ok(check_response(request(url)?.send()?, url)?.get_html()?)
}

pub fn series_key(url: &str) -> Option<String> {
	let path = url
		.strip_prefix(URL)
		.unwrap_or(url)
		.strip_prefix("/series/")?;
	let slug = path
		.split(['/', '?', '#'])
		.next()
		.filter(|slug| !slug.trim().is_empty())?;
	Some(format!("/series/{slug}"))
}

pub fn resolve_series_key(key: &str) -> Result<String> {
	if key.parse::<i32>().is_ok() {
		// Existing libraries retain the previous API's numeric keys.
		let url = format!("https://api.eternalmangas.org/api/post?postId={key}");
		let data: models::LegacyPostResponse = check_response(
			request(&url)?.header("Accept", "application/json").send()?,
			&url,
		)?
		.get_json_owned()?;
		series_key(&format!("/series/{}", data.post.slug)).ok_or(error!("Invalid series key"))
	} else {
		series_key(key).ok_or(error!("Invalid series key"))
	}
}

pub fn manga_from_card(card: &Element, label_novels: bool) -> Option<Manga> {
	let href = card
		.attr("href")
		.or_else(|| card.select_first("a[href*='/series/']")?.attr("href"))?;
	let key = series_key(&href)?;
	let image = card.select_first("img")?;
	let title = image
		.attr("alt")
		.filter(|title| !title.trim().is_empty())
		.or_else(|| {
			card.select_first(".display-6, h3, .em-series-catalog-title")?
				.text()
		})?;
	let title = title
		.trim()
		.strip_prefix("Portada de ")
		.unwrap_or(title.trim());
	if title.is_empty() {
		return None;
	}
	Some(Manga {
		key,
		title: display_title(
			title,
			card.select_first(".em-series-type-badge")
				.and_then(|kind| kind.text())
				.is_some_and(|kind| kind.to_lowercase().contains("novel")),
			label_novels,
		),
		cover: image.attr("src"),
		..Default::default()
	})
}

pub fn basic(html: &Document, key: String, label_novels: bool) -> Result<Manga> {
	let title = html
		.select_first("main h1")
		.and_then(|title| title.text())
		.filter(|title| !title.trim().is_empty())
		.ok_or(error!("Series not found"))?;
	Ok(Manga {
		key,
		title: display_title(
			&title,
			labelled(html, "Tipo:").is_some_and(|kind| kind.to_lowercase().contains("novel")),
			label_novels,
		),
		cover: html
			.select_first(".em-cover-lg")
			.and_then(|cover| cover.attr("src")),
		..Default::default()
	})
}

pub fn search(html: &Document, label_novels: bool) -> MangaPageResult {
	MangaPageResult {
		entries: html
			.select("a.em-series-catalog-card")
			.into_iter()
			.flatten()
			.filter_map(|card| manga_from_card(&card, label_novels))
			.collect(),
		has_next_page: html.select_first("a[rel=next]").is_some(),
	}
}

fn labelled(html: &Document, label: &str) -> Option<String> {
	html.select(".col-lg-3 li:has(strong)")?.find_map(|row| {
		row.text()?
			.strip_prefix(label)
			.map(|text| text.trim().into())
			.filter(|text: &String| !text.is_empty())
	})
}

fn names(text: &str) -> Option<Vec<String>> {
	let mut names: Vec<String> = Vec::new();
	for name in text.split([',', '&']).map(str::trim) {
		if !name.is_empty() && name != "-" && !names.iter().any(|existing| existing == name) {
			names.push(name.into());
		}
	}
	(!names.is_empty()).then_some(names)
}

pub fn details(html: &Document, key: String, label_novels: bool) -> Result<Manga> {
	let mut tags: Vec<String> = html
		.select("#tab-sinopsis .em-badge")
		.into_iter()
		.flatten()
		.filter_map(|tag| tag.text())
		.map(|tag| tag.trim().into())
		.filter(|tag: &String| !tag.is_empty())
		.collect();
	tags.sort_unstable();
	tags.dedup();
	let series_type = labelled(html, "Tipo:").map(|kind| kind.to_lowercase());
	let adult = series_type
		.as_deref()
		.is_some_and(|kind| kind.contains("19") || kind.contains("sin censura"));
	let content_rating = content_rating(&tags, adult);
	Ok(Manga {
		description: html
			.select_first("#resume-content")
			.and_then(|text| text.html())
			.and_then(|content| markdown::parse(&content).ok()),
		authors: labelled(html, "Autor:").as_deref().and_then(names),
		artists: labelled(html, "Artista:").as_deref().and_then(names),
		status: match labelled(html, "Estado:")
			.map(|status| status.to_lowercase())
			.as_deref()
		{
			Some("en emisión" | "próximamente") => MangaStatus::Ongoing,
			Some("finalizado") => MangaStatus::Completed,
			Some("pausado") => MangaStatus::Hiatus,
			Some("cancelado" | "abandonado") => MangaStatus::Cancelled,
			_ => MangaStatus::Unknown,
		},
		viewer: match series_type.as_deref() {
			Some("manga") => Viewer::RightToLeft,
			Some("novela" | "novela visual") | None => Viewer::Unknown,
			_ => Viewer::Webtoon,
		},
		content_rating,
		tags: (!tags.is_empty()).then_some(tags),
		url: Some(format!("{URL}{key}")),
		..basic(html, key, label_novels)?
	})
}

pub fn date(element: &Element) -> Option<i64> {
	if let Some(date) = element
		.select_first("time")
		.and_then(|time| time.attr("datetime"))
	{
		return chrono::DateTime::parse_from_rfc3339(&date)
			.ok()
			.map(|date| date.timestamp());
	}
	let text = element
		.select_first("time, .em-time-ago, .text-muted.small")?
		.text()?;
	let (amount, unit) = text.trim().strip_prefix("hace ")?.split_once(' ')?;
	let amount: i64 = amount.parse().ok()?;
	let seconds = match unit {
		"segundo" | "segundos" => 1,
		"minuto" | "minutos" => 60,
		"hora" | "horas" => 3600,
		"día" | "días" => 86400,
		"semana" | "semanas" => 604800,
		"mes" | "meses" => 2592000,
		"año" | "años" => 31536000,
		_ => return None,
	};
	Some(current_date() - amount * seconds)
}

pub fn chapter(link: &Element) -> Result<Chapter> {
	let href = link.attr("href").ok_or(error!("Missing chapter link"))?;
	let key = href
		.strip_prefix(URL)
		.unwrap_or(&href)
		.split(['?', '#'])
		.next()
		.ok_or(error!("Invalid chapter link"))?
		.trim_end_matches('/');
	if series_key(key).is_none() || key.matches('/').count() != 3 {
		bail!("Invalid chapter link");
	}
	let label = link
		.select_first(".fw-semibold")
		.and_then(|label| label.text())
		.or_else(|| link.text())
		.ok_or(error!("Missing chapter title"))?;
	let number: String = label
		.chars()
		.skip_while(|c| !c.is_ascii_digit())
		.take_while(|c| c.is_ascii_digit() || *c == '.')
		.collect();
	let chapter_number = number.parse().ok();
	Ok(Chapter {
		key: key.into(),
		url: Some(format!("{URL}{key}")),
		title: chapter_number.is_none().then(|| label.trim().into()),
		chapter_number,
		date_uploaded: date(link),
		..Default::default()
	})
}

pub fn chapters(html: &Document) -> Result<Vec<Chapter>> {
	let list = html
		.select_first("#chapter-list")
		.ok_or(error!("Chapter list not found"))?;
	list.select(".em-chapter-row > a")
		.into_iter()
		.flatten()
		.map(|link| chapter(&link))
		.collect()
}

pub fn pages(html: &Document) -> Result<Vec<Page>> {
	if let Some(content) = html
		.select_first("#em-reader-pages .novel-content")
		.and_then(|content| content.html())
	{
		return Ok(vec![Page {
			content: PageContent::text(markdown::parse(&content)?),
			..Default::default()
		}]);
	}
	let images: Vec<_> = html
		.select("#em-reader-pages img.em-reader-img")
		.into_iter()
		.flatten()
		.collect();
	if images.is_empty() {
		bail!("Chapter has no readable pages");
	}
	images
		.into_iter()
		.map(|image| {
			let url = image
				.attr("data-src")
				.or_else(|| image.attr("src"))
				.filter(|url| !url.trim().is_empty() && !url.starts_with("data:"))
				.ok_or(error!("Missing reader image"))?;
			Ok(Page {
				content: PageContent::url(url),
				..Default::default()
			})
		})
		.collect()
}

pub fn display_title(title: &str, novel: bool, label: bool) -> String {
	let title = title.trim();
	let suffix = ["(Novel)", "[Novel]", "(Novela)", "[Novela]"]
		.into_iter()
		.find(|suffix| title.ends_with(suffix));
	let novel = novel || suffix.is_some();
	let title = suffix.map_or(title, |suffix| title.trim_end_matches(suffix).trim_end());
	if novel && label {
		format!("{title} (Novel)")
	} else {
		title.into()
	}
}

pub fn content_rating(tags: &[String], adult: bool) -> ContentRating {
	if adult
		|| tags.iter().any(|tag| {
			matches!(
				tag.to_lowercase().as_str(),
				"+18" | "+19" | "adulto" | "smut" | "yaoi"
			)
		}) {
		ContentRating::NSFW
	} else if tags.iter().any(|tag| tag.eq_ignore_ascii_case("ecchi")) {
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	}
}
