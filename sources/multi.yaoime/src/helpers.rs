use crate::{
	BASE_URL, cipher,
	models::{ErrorResponse, ReaderChapter, ReaderResponse, SiteSettings, split_label},
};
use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, MangaWithChapter, Result, Viewer,
	alloc::{String, Vec, collections::BTreeSet, format, string::ToString},
	helpers::string::PlainText,
	imports::{
		html::{Document, Element, Html},
		net::{Request, Response},
		std::{current_date, parse_date_with_options},
	},
	prelude::*,
};
use serde::de::DeserializeOwned;

pub fn manga_url(key: &str) -> String {
	format!("{BASE_URL}/series/{key}")
}

pub enum Link {
	Series(String),
	Chapter(String),
}

// Series urls append a slug to the id; chapter urls name the source series and the chapter.
pub fn parse_link(url: &str) -> Option<Link> {
	let path = url.trim().strip_prefix(BASE_URL)?.strip_prefix('/')?;
	let mut segments = path.split(['?', '#']).next()?.split('/');
	match segments.next()? {
		"series" => numeric(segments.next()?).map(|id| Link::Series(id.into())),
		"chapter" => {
			let series = numeric(segments.next()?)?;
			let id = numeric(segments.next()?)?;
			Some(Link::Chapter(format!("{series}/{id}")))
		}
		_ => None,
	}
}

fn numeric(id: &str) -> Option<&str> {
	(!id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit())).then_some(id)
}

pub fn send(url: &str, settings: &SiteSettings) -> Result<Response> {
	let response = Request::get(url)?
		.header("Cookie", &settings.cookie()?)
		.send()?;
	let status = response.status_code();
	if !(200..300).contains(&status) {
		bail!("yaoi.me returned status {status}");
	}
	Ok(response)
}

pub fn get_html(url: &str, settings: &SiteSettings) -> Result<Document> {
	Ok(send(url, settings)?.get_html()?)
}

pub fn series_key(element: &Element) -> Option<String> {
	match parse_link(&element.attr("abs:href")?)? {
		Link::Series(key) => Some(key),
		Link::Chapter(_) => None,
	}
}

// Covers are served resized for a `w` query; without one the site sends the original.
fn cover_url(src: &str) -> String {
	src.split('?').next().unwrap_or(src).into()
}

pub fn parse_status(label: &str) -> MangaStatus {
	match label.to_lowercase().as_str() {
		"ongoing" | "releasing" => MangaStatus::Ongoing,
		"completed" => MangaStatus::Completed,
		"hiatus" => MangaStatus::Hiatus,
		"cancelled" => MangaStatus::Cancelled,
		_ => MangaStatus::Unknown,
	}
}

// A card is its cover link; list rows put the title on a separate link to the same series.
pub fn parse_manga_list(root: &Element) -> Vec<Manga> {
	let mut entries: Vec<Manga> = Vec::new();
	for tile in root
		.select("a.tile[href^='/series/']")
		.into_iter()
		.flatten()
	{
		let (Some(href), Some(key)) = (tile.attr("href"), series_key(&tile)) else {
			continue;
		};
		if entries.iter().any(|manga| manga.key == key) {
			continue;
		}
		let title = tile
			.attr("title")
			.filter(|title| !title.is_empty())
			.or_else(|| {
				root.select_first(format!("a.card-title[href='{href}']"))
					.and_then(|link| link.text())
			});
		let Some(title) = title else {
			continue;
		};
		let status = tile
			.select("span.cover-labels > span")
			.into_iter()
			.flatten()
			.filter_map(|label| label.text())
			.map(|label| parse_status(&label))
			.find(|status| *status != MangaStatus::Unknown)
			.unwrap_or_default();
		entries.push(Manga {
			key,
			title,
			cover: tile
				.select_first("img")
				.and_then(|image| image.attr("abs:src"))
				.map(|src| cover_url(&src)),
			status,
			content_rating: if tile.has_class("mature-cover") {
				ContentRating::NSFW
			} else {
				ContentRating::Unknown
			},
			..Default::default()
		});
	}
	entries
}

// The title's own id and cover, wherever the url's id came from.
pub fn parse_manga(document: &Document) -> Option<Manga> {
	let key = series_key(&document.select_first("link[rel=canonical]")?)?;
	let cover = document
		.select_first("meta[property='og:image']")
		.and_then(|meta| meta.attr("content"))
		.map(|src| cover_url(&src));
	Some(Manga {
		key,
		title: document.select_first("h1")?.text()?,
		cover,
		..Default::default()
	})
}

fn credits(document: &Document) -> (Vec<String>, Vec<String>) {
	let mut authors = Vec::new();
	let mut artists = Vec::new();
	let Some(html) = document
		.select("h1 ~ div")
		.into_iter()
		.flatten()
		.find(|line| line.select_first("a[href^='/author/']").is_some())
		.and_then(|line| line.html())
	else {
		return (authors, artists);
	};
	// Each role ("Story by", "Art by", "Story & art by") precedes its names.
	for part in html.split("<span aria-hidden=\"true\"> · </span>") {
		let role = part.split('<').next().unwrap_or_default().to_lowercase();
		let names: Vec<String> = Html::parse_fragment(part)
			.ok()
			.and_then(|fragment| fragment.select("a"))
			.into_iter()
			.flatten()
			.filter_map(|link| link.text())
			.collect();
		for name in names {
			if role.contains("art") && !artists.contains(&name) {
				artists.push(name.clone());
			}
			if role.contains("story") && !authors.contains(&name) {
				authors.push(name);
			}
		}
	}
	(authors, artists)
}

// The synopsis keeps its line breaks as text, so each line becomes a paragraph.
fn synopsis(document: &Document) -> Option<String> {
	let html = document
		.select("h2.section-title")?
		.find(|heading| heading.text().as_deref() == Some("Synopsis"))?
		.parent()?
		.select_first(".prose-text")?
		.html()?;
	let fragment =
		Html::parse_fragment(format!("<p>{}</p>", html.replace('\n', "</p><p>"))).ok()?;
	let lines: Vec<String> = fragment
		.select("p")?
		.filter_map(|line| line.text())
		.filter(|line| !line.is_empty())
		.map(|line| line.escape_markdown())
		.collect();
	Some(lines.join("\n\n")).filter(|text| !text.is_empty())
}

pub fn parse_manga_details(document: &Document, manga: &mut Manga) {
	let fact = |name: &str| {
		document
			.select("dl dt")
			.into_iter()
			.flatten()
			.find(|term| term.text().as_deref() == Some(name))
			.and_then(|term| term.next())
			.and_then(|value| value.text())
	};
	let (authors, artists) = credits(document);
	// A genre and a tag can share a name.
	let mut tags: Vec<String> = Vec::new();
	for chip in document
		.select("a.chip[href^='/genre/'], a.chip[href^='/tag/']")
		.into_iter()
		.flatten()
	{
		if let Some(tag) = chip.text()
			&& !tags.contains(&tag)
		{
			tags.push(tag);
		}
	}
	if let Some(found) = parse_manga(document) {
		manga.title = found.title;
		manga.cover = found.cover;
	}
	manga.authors = Some(authors).filter(|names| !names.is_empty());
	manga.artists = Some(artists).filter(|names| !names.is_empty());
	manga.description = synopsis(document);
	manga.url = Some(manga_url(&manga.key));
	manga.tags = Some(tags).filter(|tags| !tags.is_empty());
	manga.status = fact("Status").map_or(MangaStatus::Unknown, |label| parse_status(&label));
	manga.content_rating = match document
		.select_first("span.badge.normal-case")
		.and_then(|badge| badge.text())
		.map(|label| label.to_lowercase())
		.as_deref()
	{
		Some("pornographic" | "erotica") => ContentRating::NSFW,
		Some("suggestive") => ContentRating::Suggestive,
		Some("safe") => ContentRating::Safe,
		_ => ContentRating::Unknown,
	};
	manga.viewer = match fact("Format").map(|kind| kind.to_lowercase()).as_deref() {
		Some("manhwa" | "manhua") => Viewer::Webtoon,
		Some("manga" | "doujinshi") => Viewer::RightToLeft,
		Some("oel") => Viewer::LeftToRight,
		_ => Viewer::Unknown,
	};
}

// Next.js streams the page's data as JSON strings in `self.__next_f.push` scripts; this reads
// the object that opens with `start`.
pub fn page_data<T: DeserializeOwned>(body: &str, start: &str) -> Result<T> {
	let mut flight = String::new();
	for chunk in body.split("self.__next_f.push(").skip(1) {
		if let Some(Ok((1, text))) = serde_json::Deserializer::from_str(chunk)
			.into_iter::<(u8, String)>()
			.next()
		{
			flight.push_str(&text);
		}
	}
	let at = flight.find(start).ok_or(error!("Missing page data"))?;
	serde_json::Deserializer::from_str(&flight[at..])
		.into_iter::<T>()
		.next()
		.ok_or(error!("Missing page data"))?
		.map_err(Into::into)
}

pub fn reader(key: &str) -> Result<ReaderChapter> {
	let (series, chapter) = key.split_once('/').ok_or(error!("Invalid chapter key"))?;
	let path = format!("/api/reader/chapter?s={series}&c={chapter}");
	let time = (current_date() * 1000).to_string();
	let response = Request::get(format!("{BASE_URL}{path}"))?
		.header("Accept", "application/json")
		.header("x-yf-t", &time)
		.header("x-yf-s", &cipher::sign(&path, &time))
		.send()?;
	let status = response.status_code();
	if !(200..300).contains(&status) {
		match response.get_json_owned::<ErrorResponse>() {
			Ok(body) => bail!("{}", body.error),
			Err(_) => bail!("yaoi.me returned status {status}"),
		}
	}
	Ok(response.get_json_owned::<ReaderResponse>()?.chapter)
}

pub fn parse_chapter_list(root: &Element) -> Vec<MangaWithChapter> {
	let mut seen = BTreeSet::new();
	root.select("div.grid > div")
		.into_iter()
		.flatten()
		.filter_map(|row| {
			let manga = parse_manga_list(&row).into_iter().next()?;
			let link = row.select_first("a[href^='/chapter/']")?;
			let Some(Link::Chapter(key)) = link.attr("abs:href").as_deref().and_then(parse_link)
			else {
				return None;
			};
			if !seen.insert(manga.key.clone()) {
				return None;
			}
			let (chapter_number, title) = link
				.text()
				.map_or((None, None), |label| split_label(&label));
			Some(MangaWithChapter {
				chapter: Chapter {
					key,
					title,
					chapter_number,
					date_uploaded: row
						.select_first("time[datetime]")
						.and_then(|time| time.attr("datetime"))
						.and_then(|date| {
							parse_date_with_options(
								date,
								"yyyy-MM-dd'T'HH:mm:ss.SSSXXX",
								"en_US_POSIX",
								"UTC",
							)
						}),
					..Default::default()
				},
				manga,
			})
		})
		.collect()
}
