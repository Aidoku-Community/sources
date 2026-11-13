use crate::{BASE_URL, NETWORK_ERROR, PAGE_ERROR, USER_AGENT};
use aidoku::{
	Chapter, ContentRating, Manga, MangaPageResult, MangaStatus, Page, PageContent, Result, Viewer,
	alloc::{String, Vec, format, string::ToString},
	imports::{
		html::{Document, Element},
		net::Request,
		std::parse_date_with_options,
	},
	prelude::*,
};
use serde_json::Value;

/// Fetch a page of the server-rendered catalog.
/// `path` is a `/tur` path (with optional genre segment) and `params` are
/// already-encoded query pairs, joined as `name=value` with `&`.
pub fn fetch_listing(path: &str, params: Vec<(&str, String)>) -> Result<MangaPageResult> {
	let mut url = format!("{BASE_URL}{path}");
	for (i, (name, value)) in params.iter().enumerate() {
		url.push(if i == 0 { '?' } else { '&' });
		url.push_str(name);
		url.push('=');
		url.push_str(value);
	}
	let document = request_document(&url)?;
	Ok(parse_cards(&document))
}

pub fn request_document(url: &str) -> Result<Document> {
	Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.html()
		.map_err(|_| error!("{NETWORK_ERROR}"))
}

/// Extract catalog cards from a `/tur` page. Multi-page listings render a
/// "Sonraki" (`rel="next"`) button; its presence marks another page.
pub fn parse_cards(document: &Document) -> MangaPageResult {
	let mut entries = Vec::new();
	if let Some(cards) = document.select("a.series-card") {
		for card in cards {
			let Some(key) = card
				.attr("href")
				.as_deref()
				.map(slug_from_url)
				.filter(|slug| !slug.is_empty())
			else {
				continue;
			};
			let Some(title) = card
				.select_first(".series-card-title")
				.and_then(|e| e.text())
			else {
				continue;
			};
			let cover = card.select_first("img").and_then(|img| image_url(&img));
			entries.push(Manga {
				key,
				title,
				cover,
				..Default::default()
			});
		}
	}
	let has_next_page = document.select_first("a[rel=\"next\"]").is_some();
	MangaPageResult {
		entries,
		has_next_page,
	}
}

pub fn slug_from_url(url: &str) -> String {
	url.rsplit('/').next().unwrap_or_default().to_string()
}

/// Parse series details from the JSON-LD block plus the type and status
/// from the page header.
pub fn parse_details(key: &str, document: &Document) -> Manga {
	let mut title = String::new();
	let mut cover = None;
	let mut description = None;
	let mut authors = None;
	let mut tags = None;

	if let Some(ld) = document
		.select("script[type=\"application/ld+json\"]")
		.and_then(|list| list.first())
	{
		// script contents are raw data; fall back to inner html where data
		// isn't available (the test runner doesn't implement it)
		let json = ld.data().or_else(|| ld.html()).unwrap_or_default();
		if let Ok(value) = serde_json::from_str::<Value>(json.trim())
			&& let Some(series) = value
				.get("@graph")
				.and_then(Value::as_array)
				.and_then(|list| {
					list.iter().find(|node| {
						node.get("@type").and_then(Value::as_str) == Some("ComicSeries")
					})
				}) {
			title = string_field(series, "name").unwrap_or_default();
			cover = string_field(series, "image");
			description = string_field(series, "description");
			// `author` is either a single Person or an array of them
			let names = match series.get("author") {
				Some(Value::Array(list)) => list
					.iter()
					.filter_map(|a| string_field(a, "name"))
					.collect(),
				Some(author) => string_field(author, "name").into_iter().collect(),
				None => Vec::new(),
			};
			if !names.is_empty() {
				authors = Some(names);
			}
			if let Some(genres) = series.get("genre").and_then(Value::as_array) {
				tags = Some(
					genres
						.iter()
						.filter_map(Value::as_str)
						.map(ToString::to_string)
						.collect::<Vec<_>>(),
				);
			}
		}
	}

	if title.is_empty() {
		title = document
			.select_first("h1")
			.and_then(|e| e.text())
			.unwrap_or_default();
		if title.is_empty() {
			title = key.to_string();
		}
	}
	if cover.is_none() {
		cover = document
			.select_first("meta[property=\"og:image\"]")
			.and_then(|m| m.attr("content"));
	}

	// the header kicker holds the series type and publishing status:
	// "<type> · <status>"
	let mut viewer = Viewer::Webtoon;
	let mut status = MangaStatus::Unknown;
	if let Some(kicker) = document.select_first(".series-detail-kicker") {
		let text = kicker.text().unwrap_or_default();
		for part in text.split('·').map(str::trim) {
			match fold_turkish(part).as_str() {
				"manga" => viewer = Viewer::RightToLeft,
				"devam ediyor" => status = MangaStatus::Ongoing,
				"tamamlandi" => status = MangaStatus::Completed,
				"ara verildi" => status = MangaStatus::Hiatus,
				"birakildi" | "terk edildi" => status = MangaStatus::Cancelled,
				_ => {}
			}
		}
	}

	let suggestive = tags.iter().flatten().any(|tag| {
		let tag = tag.to_lowercase();
		tag.contains("ecchi") || tag.contains("eroti")
	});
	let content_rating = if suggestive {
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	};

	Manga {
		key: key.to_string(),
		title,
		cover,
		authors,
		description,
		url: Some(format!("{BASE_URL}/{key}")),
		tags,
		status,
		content_rating,
		viewer,
		..Default::default()
	}
}

/// A non-empty string field of a JSON object.
fn string_field(value: &Value, field: &str) -> Option<String> {
	value
		.get(field)
		.and_then(Value::as_str)
		.map(str::trim)
		.filter(|s| !s.is_empty())
		.map(ToString::to_string)
}

/// Chapters are server-rendered as `div.chapter-item` elements with the
/// chapter number in a data attribute and the slug in the link href,
/// newest first (which the app expects).
pub fn parse_chapters(key: &str, document: &Document) -> Vec<Chapter> {
	let mut chapters = Vec::new();
	let Some(items) = document.select("div.chapter-item") else {
		return chapters;
	};
	for item in items {
		let Some(chapter_url) = item
			.select_first("a.chapter-item-link")
			.and_then(|a| a.attr("href"))
		else {
			continue;
		};
		let chapter_key = slug_from_url(&chapter_url);
		if chapter_key.is_empty() {
			continue;
		}
		let chapter_url = format!("{BASE_URL}/{key}/{chapter_key}");
		let number = item
			.attr("data-chapter-number")
			.and_then(|n| n.parse::<f32>().ok());
		// chapter titles always mirror the chapter number ("Bölüm 366");
		// only keep a title when the site has a real one
		let label = item.attr("data-chapter-number-label");
		let title = item
			.select_first(".chapter-title")
			.and_then(|e| e.text())
			.filter(|t| !t.is_empty() && t.strip_prefix("Bölüm ") != label.as_deref());
		// dates look like "3 Kasım 2026"
		let date_uploaded = item
			.select_first(".chapter-date")
			.and_then(|e| e.text())
			.and_then(|date| parse_date_with_options(date, "d MMMM yyyy", "tr_TR", "UTC"));
		chapters.push(Chapter {
			url: Some(chapter_url),
			key: chapter_key,
			title,
			chapter_number: number,
			date_uploaded,
			..Default::default()
		});
	}
	chapters
}

/// Lowercase and strip Turkish diacritics so "TAMAMLANDI", "Tamamlandı" and
/// "tamamlandi" all compare equal. Plain `to_lowercase` maps "I" to "i" and
/// "İ" to "i\u{307}", so both dotted and dotless forms are folded to "i".
pub fn fold_turkish(text: &str) -> String {
	text.chars()
		.flat_map(char::to_lowercase)
		.filter_map(|c| match c {
			'\u{307}' => None,
			'ı' => Some('i'),
			'ş' => Some('s'),
			'ğ' => Some('g'),
			'ü' => Some('u'),
			'ö' => Some('o'),
			'ç' => Some('c'),
			c => Some(c),
		})
		.collect()
}

/// Resolve an image attribute to an absolute https URL.
pub fn absolute_url(src: &str) -> Option<String> {
	let src = src.trim();
	if src.starts_with("https://") {
		Some(src.to_string())
	} else if let Some(rest) = src.strip_prefix("//") {
		Some(format!("https://{rest}"))
	} else if src.starts_with('/') {
		Some(format!("{BASE_URL}{src}"))
	} else {
		None
	}
}

/// The real image URL of an `img`, preferring lazy-load attributes over a
/// possible placeholder `src`.
fn image_url(img: &Element) -> Option<String> {
	["data-src", "data-lazy-src", "src"]
		.iter()
		.filter_map(|attr| img.attr(attr))
		.find_map(|src| absolute_url(&src))
}

/// Reader pages are server-rendered `img.reader-page` elements whose src
/// attributes are the page images, in order.
pub fn parse_pages(document: &Document) -> Result<Vec<Page>> {
	let Some(images) = document.select("img.reader-page") else {
		bail!("{PAGE_ERROR}");
	};
	let pages = images
		.filter_map(|image| image_url(&image))
		.map(|url| Page {
			content: PageContent::Url(url, None),
			..Default::default()
		})
		.collect::<Vec<_>>();
	if pages.is_empty() {
		bail!("{PAGE_ERROR}");
	}
	Ok(pages)
}
