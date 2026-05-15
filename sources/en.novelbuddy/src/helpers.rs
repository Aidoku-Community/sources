use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, Result,
	alloc::{String, Vec, string::ToString},
	imports::{html::Html, net::Request, std::parse_date},
	prelude::*,
};
use serde::de::DeserializeOwned;

use crate::models::{
	ApiResponse, BySlugData, ChapterListData, ChapterListItem, TitleDetail, TitleListItem,
};
use crate::{API_URL, BASE_URL, USER_AGENT};

pub fn request<T: DeserializeOwned>(url: &str) -> Result<T> {
	let response = Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.header("Accept", "application/json, text/plain, */*")
		.header("Referer", "https://novelbuddy.com/")
		.header("Origin", BASE_URL)
		.json_owned::<ApiResponse<T>>()?;
	if !response.success {
		let msg = response
			.message
			.unwrap_or_else(|| "API request failed".into());
		bail!("{msg}");
	}
	response.data.ok_or_else(|| error!("API returned no data"))
}

pub fn fetch_chapter_list(title_id: &str) -> Result<Vec<Chapter>> {
	let url = format!("{API_URL}/titles/{title_id}/chapters");
	let data: ChapterListData = request(&url)?;
	Ok(data.chapters.into_iter().map(Chapter::from).collect())
}

pub fn resolve_slug(slug: &str) -> Result<String> {
	let url = format!("{API_URL}/titles/by-slug/{slug}");
	let data: BySlugData = request(&url)?;
	parse_id_from_canonical(&data.new_url)
		.ok_or_else(|| error!("Could not parse id from {}", data.new_url))
}

impl From<TitleListItem> for Manga {
	fn from(item: TitleListItem) -> Self {
		let slug = item.slug.as_deref().unwrap_or("");
		let url = if slug.is_empty() {
			None
		} else {
			Some(format!("{BASE_URL}/{slug}"))
		};
		Manga {
			key: item.id,
			title: item.name,
			cover: item.cover,
			url,
			..Default::default()
		}
	}
}

impl From<TitleDetail> for Manga {
	fn from(detail: TitleDetail) -> Self {
		let url = detail.slug.as_deref().map(|s| format!("{BASE_URL}/{s}"));
		let description = detail
			.summary
			.as_deref()
			.map(html_to_text)
			.filter(|t| !t.is_empty());
		let status = detail
			.status
			.as_deref()
			.map(parse_status)
			.unwrap_or(MangaStatus::Unknown);
		let authors: Vec<String> = detail.authors.into_iter().map(|a| a.name).collect();
		let artists: Vec<String> = detail.artists.into_iter().map(|a| a.name).collect();
		let mut tags: Vec<String> = detail.genres.into_iter().map(|g| g.name).collect();
		for tag in detail.tags.into_iter().map(|t| t.name) {
			if !tags.iter().any(|t| t == &tag) {
				tags.push(tag);
			}
		}
		let rating = content_rating(detail.is_adult, &tags);
		Manga {
			key: detail.id,
			title: detail.name,
			cover: detail.cover,
			url,
			description,
			authors: (!authors.is_empty()).then_some(authors),
			artists: (!artists.is_empty()).then_some(artists),
			tags: (!tags.is_empty()).then_some(tags),
			status,
			content_rating: rating,
			..Default::default()
		}
	}
}

impl From<ChapterListItem> for Chapter {
	fn from(item: ChapterListItem) -> Self {
		let chapter_number = parse_chapter_number(&item.name);
		let date_uploaded = item.updated_at.as_deref().and_then(parse_iso_date);
		let url = item.url.as_deref().map(absolute_url);
		Chapter {
			key: item.id,
			title: Some(item.name),
			chapter_number,
			date_uploaded,
			url,
			..Default::default()
		}
	}
}

pub fn parse_status(value: &str) -> MangaStatus {
	match value.to_ascii_lowercase().as_str() {
		"ongoing" => MangaStatus::Ongoing,
		"completed" => MangaStatus::Completed,
		"hiatus" => MangaStatus::Hiatus,
		"cancelled" | "canceled" => MangaStatus::Cancelled,
		_ => MangaStatus::Unknown,
	}
}

pub fn content_rating(is_adult: i32, tags: &[String]) -> ContentRating {
	if is_adult != 0 {
		return ContentRating::NSFW;
	}
	for tag in tags {
		match tag.as_str() {
			"Adult" | "Smut" | "Mature" | "Ecchi" | "Lolicon" | "Yaoi" | "Yuri" => {
				return ContentRating::Suggestive;
			}
			_ => {}
		}
	}
	ContentRating::Safe
}

pub fn absolute_url(path_or_url: &str) -> String {
	if path_or_url.starts_with("http") {
		path_or_url.into()
	} else if path_or_url.starts_with('/') {
		format!("{BASE_URL}{path_or_url}")
	} else {
		format!("{BASE_URL}/{path_or_url}")
	}
}

pub fn parse_iso_date(value: &str) -> Option<i64> {
	parse_date(value, "yyyy-MM-dd'T'HH:mm:ss.SSSXXX")
}

pub fn parse_chapter_number(name: &str) -> Option<f32> {
	// Chapter-name formats vary across titles ("Chapter 5", "Chapter: 5",
	// "Chapter ’5", "Chapter 12.5"), but the number is always the first numeric
	// run. Keep a single '.' for decimal (bonus) chapters.
	let mut num = String::new();
	let mut seen_dot = false;
	for ch in name.chars() {
		if ch.is_ascii_digit() {
			num.push(ch);
		} else if ch == '.' && !seen_dot && !num.is_empty() {
			seen_dot = true;
			num.push(ch);
		} else if !num.is_empty() {
			break;
		}
	}
	num.parse().ok()
}

pub fn parse_id_from_canonical(new_url: &str) -> Option<String> {
	let trimmed = new_url.trim_start_matches("/titles/");
	let id = trimmed.split('-').next()?;
	if id.len() == 8 && id.chars().all(|c| c.is_ascii_alphanumeric()) {
		Some(id.into())
	} else {
		None
	}
}

/// Convert NovelBuddy's chapter/description HTML to plain text for
/// `PageContent::Text`. The API wraps prose in `<p>` paragraphs alongside
/// empty ad-placement divs; selecting `p` ignores the latter, and the HTML
/// parser handles entity decoding and nested inline tags.
pub fn html_to_text(html: &str) -> String {
	let Ok(doc) = Html::parse_fragment(html) else {
		return String::new();
	};
	doc.select("p")
		.map(|els| {
			els.filter_map(|el| {
				let text = el.text()?;
				let trimmed = text.trim();
				(!trimmed.is_empty()).then(|| trimmed.to_string())
			})
			.collect::<Vec<_>>()
			.join("\n\n")
		})
		.unwrap_or_default()
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku::alloc::string::ToString;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn extracts_paragraphs() {
		let html = "\n  <div><p> Hello world.</p><p>Second paragraph.</p>\n<div style=\"text-align:center\"><div></div></div></div>";
		let out = html_to_text(html);
		assert_eq!(out, "Hello world.\n\nSecond paragraph.");
	}

	#[aidoku_test]
	fn decodes_entities() {
		let html = "<p>Tom &amp; Jerry &mdash; together</p>";
		let out = html_to_text(html);
		assert_eq!(out, "Tom & Jerry — together");
	}

	#[aidoku_test]
	fn strips_inner_tags() {
		let html = "<p>A <em>bold</em> claim</p>";
		let out = html_to_text(html);
		assert_eq!(out, "A bold claim");
	}

	#[aidoku_test]
	fn parses_canonical_id() {
		assert_eq!(
			parse_id_from_canonical("/titles/VYPGVZ8z-shadow-slave"),
			Some("VYPGVZ8z".to_string())
		);
		assert_eq!(parse_id_from_canonical("/titles/garbage"), None);
	}

	#[aidoku_test]
	fn parses_chapter_number() {
		assert_eq!(
			parse_chapter_number("Chapter 2995 Time to Return"),
			Some(2995.0)
		);
		assert_eq!(parse_chapter_number("Chapter 12.5: Bonus"), Some(12.5));
		// Verified real on the live API (rare but present) — do not drop decimal
		// handling: the slug for this is `chapter-374-5` (ambiguous), the name is not.
		assert_eq!(parse_chapter_number("Chapter 374.5"), Some(374.5));
		assert_eq!(parse_chapter_number("Prologue"), None);
		// Live API also returns these formats (verified on the Shadow Slave list):
		assert_eq!(
			parse_chapter_number("Chapter: 2234 Darkness Falls"),
			Some(2234.0)
		);
		assert_eq!(
			parse_chapter_number("Chapter ’2362 Hunter and Prey"),
			Some(2362.0)
		);
		assert_eq!(parse_chapter_number("Chapter One"), None);
	}
}
