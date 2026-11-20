use crate::models::*;
use aidoku::{
	Chapter, ContentRating, MangaStatus, Result, Viewer,
	alloc::{String, Vec},
	imports::{
		html::Html,
		std::{get_utc_offset, parse_date},
	},
	prelude::*,
};
use chrono::{DateTime, Datelike, NaiveDate};
use serde_json::Value;

// The wrapper works in the test runner too, where a fragment has no body to select
pub fn html_text(value: &str) -> Option<String> {
	if value.contains(['&', '<']) {
		Html::parse(format!("<div id=text>{value}</div>"))
			.ok()?
			.select_first("#text")?
			.text()
	} else {
		Some(value.trim().into())
	}
	.filter(|text| !text.is_empty())
}

pub fn description_text(description: &str, markdown: bool) -> Option<String> {
	let mut text = String::from(description);
	if text.contains("<br") || text.contains("<p") {
		text = text.replace(['\r', '\n'], " ");
	}
	for (tag, value) in [
		("<br>", "\n"),
		("<br/>", "\n"),
		("<br />", "\n"),
		("</p>", "\n\n"),
	] {
		text = text.replace(tag, value);
	}
	if markdown {
		for (tags, marker) in [
			(["<b>", "</b>", "<strong>", "</strong>"], "**"),
			(["<i>", "</i>", "<em>", "</em>"], "*"),
		] {
			for tag in tags {
				text = text.replace(tag, marker);
			}
		}
	}
	let mut paragraphs: Vec<String> = Vec::new();
	let mut lines: Vec<String> = Vec::new();
	for line in text.split('\n').chain([""]) {
		if let Some(line) = html_text(line) {
			lines.push(line);
		} else if !lines.is_empty() {
			paragraphs.push(lines.join(if markdown { "  \n" } else { "\n" }));
			lines.clear();
		}
	}
	(!paragraphs.is_empty()).then(|| paragraphs.join("\n\n"))
}

// The site shows the English name when a series has one
pub fn display_title(english: Option<&str>, name: &str) -> Result<String> {
	english
		.and_then(html_text)
		.or_else(|| html_text(name))
		.ok_or(error!("Series has no title"))
}

// Tags arrive as slugs such as `format:long_strip`
pub fn tag_names(tags: Vec<String>) -> Vec<String> {
	let mut names: Vec<String> = tags
		.into_iter()
		.map(|tag| tag_name(&tag))
		.filter(|name| !name.is_empty())
		.collect();
	names.sort_unstable_by_key(|name| name.to_lowercase());
	names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
	names
}

pub fn tag_name(slug: &str) -> String {
	slug.rsplit(':')
		.next()
		.unwrap_or_default()
		.trim_end_matches("-magazine")
		.replace('_', " ")
		.split_whitespace()
		.map(|word| {
			let mut chars = word.chars();
			chars
				.next()
				.map(|first| first.to_uppercase().chain(chars).collect::<String>())
				.unwrap_or_default()
		})
		.collect::<Vec<_>>()
		.join(" ")
}

// Korean and Chinese comics are long strips; a series without a type falls back to its country
pub fn viewer(kind: Option<&str>, country: Option<&str>, tags: &[String]) -> Viewer {
	let kind = kind.map(str::to_lowercase);
	if tags
		.iter()
		.any(|tag| tag.eq_ignore_ascii_case("Long Strip") || tag.eq_ignore_ascii_case("Webtoons"))
	{
		return Viewer::Webtoon;
	}
	match (kind.as_deref(), country) {
		(Some("manhwa" | "manhua"), _) => Viewer::Webtoon,
		(Some("manga"), _) => Viewer::RightToLeft,
		(None, Some("KR" | "CN")) => Viewer::Webtoon,
		(None, Some("JP")) => Viewer::RightToLeft,
		_ => Viewer::Unknown,
	}
}

pub fn manga_url(key: &str) -> String {
	format!("{BASE_URL}/manga/{key}")
}

pub fn chapter_url(manga_key: &str, number: &str) -> String {
	format!("{BASE_URL}/manga/{manga_key}/chapter-{number}-sub")
}

// Thumbnails are paths on the image host, or full urls for covers taken from elsewhere
pub fn cover_url(thumbnail: Option<&str>) -> Option<String> {
	let thumbnail = thumbnail.map(str::trim).filter(|path| !path.is_empty())?;
	if is_absolute(thumbnail) {
		Some(thumbnail.into())
	} else {
		Some(format!(
			"{CDN_URL}/aln.youtube-anime.com/{}?w=250",
			thumbnail.trim_start_matches('/')
		))
	}
}

pub fn content_rating(genres: &[String], is_adult: Option<bool>) -> ContentRating {
	let has = |names: &[&str]| {
		genres
			.iter()
			.any(|genre| names.iter().any(|name| genre.eq_ignore_ascii_case(name)))
	};
	if is_adult == Some(true) || has(&["Adult", "Hentai", "Smut", "Yaoi"]) {
		ContentRating::NSFW
	} else if has(&["Ecchi", "Mature"]) {
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	}
}

pub fn parse_status(status: Option<&str>) -> MangaStatus {
	let Some(status) = status.map(str::to_lowercase) else {
		return MangaStatus::Unknown;
	};
	if status.contains("releasing") || status.contains("ongoing") {
		MangaStatus::Ongoing
	} else if status.contains("finished") || status.contains("completed") {
		MangaStatus::Completed
	} else if status.contains("hiatus") {
		MangaStatus::Hiatus
	} else if status.contains("cancel") {
		MangaStatus::Cancelled
	} else {
		MangaStatus::Unknown
	}
}

pub fn apply_image_quality(url: &str, quality: &str) -> String {
	if quality == "original" {
		return url.into();
	}
	match url
		.strip_prefix("https://")
		.or_else(|| url.strip_prefix("http://"))
	{
		Some(rest) => {
			let path = rest.split('#').next().unwrap_or(rest);
			let separator = if path.contains('?') { '&' } else { '?' };
			format!("{CDN_URL}/{path}{separator}w={quality}")
		}
		None => url.into(),
	}
}

// Date parts are UTC and count months from zero
pub fn date_from_parts(parts: &DateParts) -> Option<i64> {
	let part = |value: Option<i32>, default: i32| u32::try_from(value.unwrap_or(default)).ok();
	let date =
		NaiveDate::from_ymd_opt(parts.year?, part(parts.month, 0)? + 1, part(parts.date, 1)?)?
			.and_hms_opt(
				part(parts.hour, 0)?,
				part(parts.minute, 0)?,
				part(parts.second, 0)?,
			)?;
	Some(date.and_utc().timestamp())
}

// The site names its season rows by calendar quarter: Winter is January to March
pub fn season_and_year(timestamp: i64) -> (&'static str, i32) {
	let date = DateTime::from_timestamp(timestamp, 0).unwrap_or_default();
	let season = ["Winter", "Spring", "Summer", "Fall"][date.month0() as usize / 3];
	(season, date.year())
}

// The site's update days run from local midnight to the second before the next
pub fn day_range(days_ago: i64, now: i64) -> (i64, i64) {
	let offset = get_utc_offset();
	let start = (now - offset).div_euclid(86400) * 86400 + offset - days_ago * 86400;
	(start, start + 86399)
}

pub fn has_next_page(page: i32, total: Option<i32>) -> bool {
	total.is_some_and(|total| page * PAGE_SIZE < total)
}

// Episode numbers arrive as numbers or strings
fn is_episode(value: &Value, number: &str) -> bool {
	match value {
		Value::String(value) => value == number,
		Value::Number(value) => value.as_f64() == number.parse().ok(),
		_ => false,
	}
}

// A chapter note reads "[S2] ep. 12 - Title"; keep only the title
fn chapter_title(notes: &str) -> Option<String> {
	let trimmed = notes.trim();
	let without_season = match trimmed
		.strip_prefix('[')
		.and_then(|rest| rest.split_once(']'))
	{
		Some((_, rest)) => rest.trim_start(),
		None => trimmed,
	};
	let title = strip_ep_prefix(without_season);
	title
		.chars()
		.any(|c| c.is_ascii_alphabetic())
		.then(|| html_text(title))
		.flatten()
}

fn strip_ep_prefix(input: &str) -> &str {
	if !input
		.get(..2)
		.is_some_and(|prefix| prefix.eq_ignore_ascii_case("ep"))
	{
		return input;
	}
	let rest = input[2..].trim_start_matches('.').trim_start();
	let digits = rest
		.find(|c: char| !(c.is_ascii_digit() || c == '.'))
		.unwrap_or(rest.len());
	if digits == 0 {
		return input;
	}
	rest[digits..]
		.trim_start()
		.trim_start_matches('-')
		.trim_start()
}

// The chapter list arrives newest first
pub fn parse_chapters(data: ChaptersData, manga_key: &str) -> Result<Vec<Chapter>> {
	let infos = data.episode_infos.unwrap_or_default();
	Ok(data
		.manga
		.ok_or(error!("Series not found"))?
		.available_chapters_detail
		.and_then(|detail| detail.sub)
		.unwrap_or_default()
		.into_iter()
		.map(|number| {
			let info = infos
				.iter()
				.find(|info| is_episode(&info.episode_id_num, &number));
			Chapter {
				title: info
					.and_then(|info| info.notes.as_deref())
					.and_then(chapter_title),
				chapter_number: number.parse().ok(),
				date_uploaded: info
					.and_then(|info| info.upload_dates.as_ref())
					.and_then(|dates| dates.sub.as_deref())
					.and_then(|date| date.get(..19))
					.and_then(|date| parse_date(date, "yyyy-MM-dd'T'HH:mm:ss")),
				url: Some(chapter_url(manga_key, &number)),
				key: number,
				..Default::default()
			}
		})
		.collect())
}

fn is_absolute(url: &str) -> bool {
	url.starts_with("http://") || url.starts_with("https://")
}

pub fn parse_page_urls(pages: &ChapterPages, quality: &str) -> Result<Vec<String>> {
	let first = pages
		.edges
		.first()
		.ok_or(error!("The reader did not produce any pages"))?;
	let edge = pages
		.edges
		.iter()
		.find(|edge| {
			edge.picture_url_head.is_some()
				|| edge
					.picture_urls
					.iter()
					.flatten()
					.any(|picture| picture.url().is_some_and(is_absolute))
		})
		.unwrap_or(first);
	let image_host = match edge.picture_url_head.as_deref() {
		Some(server) if is_absolute(server) => format!("{}/", server.trim_end_matches('/')),
		Some(server) => format!("https://{}/", server.trim_end_matches('/')),
		None => "https://ytimgf.youtube-anime.com/".into(),
	};
	let urls = edge
		.picture_urls
		.iter()
		.flatten()
		.map(|picture| {
			let url = picture
				.url()
				.filter(|url| !url.is_empty())
				.ok_or(error!("A page has no image url"))?;
			let url = if is_absolute(url) {
				url.into()
			} else {
				format!("{image_host}{}", url.trim_start_matches('/'))
			};
			Ok(apply_image_quality(&url, quality))
		})
		.collect::<Result<Vec<_>>>()?;
	if urls.is_empty() {
		bail!("The reader did not produce any pages");
	}
	Ok(urls)
}
