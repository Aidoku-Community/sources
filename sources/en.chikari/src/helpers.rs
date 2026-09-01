use crate::models::{ChapterItem, ChapterListResponse, ListResponse, NovelDetail, NovelListItem};
use crate::{BASE_URL, USER_AGENT};
use aidoku::{
	Chapter, ContentRating, FilterValue, Manga, MangaPageResult, MangaStatus, Result, Viewer,
	alloc::{String, Vec, string::ToString},
	helpers::{string::PlainText, uri::QueryParameters},
	imports::defaults::defaults_get,
	imports::{net::Request, std::parse_date},
	prelude::*,
};
use serde::de::DeserializeOwned;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ContentType {
	Novel,
	Series,
}

pub const SERIES_KEY_PREFIX: &str = "series:";
pub const CONTENT_TYPE_SETTING: &str = "content_type";

impl ContentType {
	pub fn api_base(self) -> &'static str {
		match self {
			Self::Novel => "/api/novels",
			Self::Series => "/api/series",
		}
	}

	pub fn web_base(self) -> &'static str {
		match self {
			Self::Novel => "/novels",
			Self::Series => "/series",
		}
	}

	pub fn from_url(url: &str) -> Self {
		if url.contains("/series/") {
			Self::Series
		} else {
			Self::Novel
		}
	}

	pub fn from_segment(segment: &str) -> Self {
		if segment == "series" {
			Self::Series
		} else {
			Self::Novel
		}
	}
}

pub fn deep_link_manga_key(content_type: ContentType, slug: &str) -> String {
	match content_type {
		ContentType::Novel => slug.into(),
		ContentType::Series => format!("{SERIES_KEY_PREFIX}{slug}"),
	}
}

pub fn is_typed_series_key(key: &str) -> bool {
	key.starts_with(SERIES_KEY_PREFIX)
}

pub fn decode_manga_key<'a>(key: &'a str, url: Option<&str>) -> (ContentType, &'a str) {
	if is_typed_series_key(key) {
		return (ContentType::Series, &key[SERIES_KEY_PREFIX.len()..]);
	}
	let content_type = url.map_or(ContentType::Novel, ContentType::from_url);
	(content_type, key)
}

pub fn content_type_from_setting() -> ContentType {
	content_type_from_setting_value(defaults_get::<String>(CONTENT_TYPE_SETTING).as_deref())
}

pub fn content_type_from_setting_value(value: Option<&str>) -> ContentType {
	match value {
		Some("comics") | Some("series") => ContentType::Series,
		_ => ContentType::Novel,
	}
}

pub fn request<T: DeserializeOwned>(url: &str) -> Result<T> {
	Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.header("Accept", "application/json")
		.header("Referer", BASE_URL)
		.header("Origin", BASE_URL)
		.json_owned::<T>()
}

pub fn list_url(
	content_type: ContentType,
	sort: &str,
	limit: u32,
	offset: u32,
	query: Option<&str>,
	filters: &[FilterValue],
) -> String {
	let mut qs = QueryParameters::new();
	qs.push("sort", Some(sort));
	qs.push("limit", Some(&limit.to_string()));
	qs.push("offset", Some(&offset.to_string()));
	if let Some(query) = query {
		qs.push("q", Some(query));
	}
	for filter in filters {
		if let FilterValue::MultiSelect {
			id,
			included,
			excluded,
		} = filter && id == "genres"
		{
			for genre in included {
				qs.push("genre", Some(genre));
			}
			for genre in excluded {
				qs.push("genre_exclude", Some(genre));
			}
		}
	}
	format!("{BASE_URL}{}?{qs}", content_type.api_base())
}

pub fn has_next(len: usize, total: u32, offset: u32, limit: u32) -> bool {
	if total > 0 {
		offset.saturating_add(len as u32) < total
	} else {
		len >= limit as usize
	}
}

pub fn effective_limit(server_limit: u32, requested_limit: u32) -> u32 {
	if server_limit == 0 {
		requested_limit
	} else {
		server_limit
	}
}

pub fn effective_offset(server_offset: u32, requested_offset: u32) -> u32 {
	if server_offset == 0 {
		requested_offset
	} else {
		server_offset
	}
}

pub fn manga_page_result(
	data: ListResponse,
	content_type: ContentType,
	offset: u32,
	limit: u32,
) -> MangaPageResult {
	let response_offset = effective_offset(data.offset, offset);
	let has_next_page = has_next(
		data.items.len(),
		data.total,
		response_offset,
		effective_limit(data.limit, limit),
	);
	MangaPageResult {
		entries: data
			.items
			.into_iter()
			.map(|item| manga_from_list(item, content_type))
			.collect(),
		has_next_page,
	}
}

pub fn manga_from_list(item: NovelListItem, content_type: ContentType) -> Manga {
	let slug = item.slug;
	let key = deep_link_manga_key(content_type, &slug);
	let url = Some(format!("{BASE_URL}{}/{slug}", content_type.web_base()));
	Manga {
		key,
		title: item.title,
		cover: item.cover_url,
		url,
		content_rating: list_content_rating(item.is_nsfw),
		status: item
			.status
			.as_deref()
			.map(parse_status)
			.unwrap_or_default(),
		..Default::default()
	}
}

pub fn list_content_rating(is_nsfw: bool) -> ContentRating {
	if is_nsfw {
		ContentRating::NSFW
	} else {
		ContentRating::Unknown
	}
}

pub fn manga_from_detail(detail: NovelDetail, content_type: ContentType) -> Manga {
	let slug = detail.slug;
	let key = deep_link_manga_key(content_type, &slug);
	let url = Some(format!("{BASE_URL}{}/{slug}", content_type.web_base()));
	let content_rating = content_rating(detail.is_nsfw, &detail.genres, &detail.tags);
	let mut tags = Vec::new();
	for name in detail.genres.into_iter().map(|g| g.name).chain(
		detail
			.tags
			.into_iter()
			.filter(|t| !t.is_spoiler)
			.map(|t| t.name),
	) {
		if !tags.iter().any(|tag| tag == &name) {
			tags.push(name);
		}
	}
	let mut authors = Vec::new();
	let mut artists = Vec::new();
	for person in detail.authors {
		let target = match person
			.role
			.as_deref()
			.map(str::to_ascii_lowercase)
			.as_deref()
		{
			Some("artist") | Some("artists") | Some("illustrator") | Some("illustrators") => {
				&mut artists
			}
			_ => &mut authors,
		};
		if !target.iter().any(|name| name == &person.name) {
			target.push(person.name);
		}
	}
	Manga {
		key,
		title: detail.title,
		cover: detail.cover_url,
		url,
		description: detail.description.filter(|s| !s.trim().is_empty()),
		authors: (!authors.is_empty()).then_some(authors),
		artists: (!artists.is_empty()).then_some(artists),
		tags: (!tags.is_empty()).then_some(tags),
		status: detail
			.status
			.as_deref()
			.map(parse_status)
			.unwrap_or_default(),
		content_rating,
		viewer: viewer_for_series(detail.reading_mode.as_deref(), detail.kind.as_deref()),
		..Default::default()
	}
}

pub fn viewer_for_series(reading_mode: Option<&str>, kind: Option<&str>) -> Viewer {
	let mode = reading_mode.map(str::to_ascii_lowercase);
	let kind = kind.map(str::to_ascii_lowercase);
	if matches!(
		mode.as_deref(),
		Some("strip") | Some("long_strip") | Some("webtoon") | Some("vertical")
	) {
		return Viewer::Webtoon;
	}
	match kind.as_deref() {
		Some("manga") => Viewer::RightToLeft,
		Some("manhwa") | Some("manhua") => Viewer::Webtoon,
		_ => Viewer::Unknown,
	}
}

pub fn content_rating(
	is_nsfw: bool,
	genres: &[crate::models::Named],
	tags: &[crate::models::Tag],
) -> ContentRating {
	if is_nsfw {
		return ContentRating::NSFW;
	}
	let is_mature = genres.iter().any(|genre| is_mature_signal(&genre.name))
		|| tags.iter().any(|tag| is_mature_signal(&tag.name));
	if is_mature {
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	}
}

fn is_mature_signal(value: &str) -> bool {
	value.eq_ignore_ascii_case("mature")
		|| value.eq_ignore_ascii_case("r-18")
		|| value.eq_ignore_ascii_case("r18")
		|| value.eq_ignore_ascii_case("adult")
		|| value.eq_ignore_ascii_case("ecchi")
		|| value.eq_ignore_ascii_case("smut")
		|| value.eq_ignore_ascii_case("yaoi")
		|| value.eq_ignore_ascii_case("yuri")
}

pub fn parse_status(status: &str) -> MangaStatus {
	match status.to_ascii_lowercase().as_str() {
		"releasing" | "ongoing" => MangaStatus::Ongoing,
		"completed" => MangaStatus::Completed,
		"hiatus" => MangaStatus::Hiatus,
		"cancelled" | "canceled" => MangaStatus::Cancelled,
		_ => MangaStatus::Unknown,
	}
}

pub fn chapter_key(number: f32) -> String {
	number.to_string()
}

pub fn chapter_from_item(item: ChapterItem) -> Chapter {
	let key = chapter_key(item.number);
	Chapter {
		key,
		title: item.title.filter(|s| !s.trim().is_empty()),
		chapter_number: Some(item.number),
		date_uploaded: item.created_at.as_deref().and_then(parse_iso_date),
		..Default::default()
	}
}

pub fn fetch_chapters(slug: &str, content_type: ContentType) -> Result<Vec<Chapter>> {
	const CHAPTER_PAGE_SIZE: u32 = 500;
	let mut result = Vec::new();
	let mut offset = 0u32;
	loop {
		let data: ChapterListResponse = request(&format!(
			"{BASE_URL}{}/{slug}/chapters?order=desc&limit={CHAPTER_PAGE_SIZE}&offset={offset}",
			content_type.api_base()
		))?;
		let count = data.items.len();
		if count == 0 {
			break;
		}
		let total = data.total;
		let response_offset = effective_offset(data.offset, offset);
		result.extend(data.items.into_iter().map(chapter_from_item));
		if !has_next(
			count,
			total,
			response_offset,
			effective_limit(data.limit, CHAPTER_PAGE_SIZE),
		) {
			break;
		}
		let next_offset = response_offset.saturating_add(count as u32);
		if next_offset <= offset {
			break;
		}
		offset = next_offset;
	}
	Ok(result)
}

pub fn parse_iso_date(value: &str) -> Option<i64> {
	// Parse the base datetime as UTC, then apply any embedded timezone offset so the result is
	// normalized to UTC (e.g. 2026-02-21T22:08:14-05:00 -> 03:08:14 UTC).
	let base = parse_date(value.get(..19)?, "yyyy-MM-ddTHH:mm:ss")?;
	// Remaining part after the base datetime: optional fractional seconds and/or timezone offset.
	let offset = value
		.get(19..)?
		.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.');
	if offset.is_empty() || offset == "Z" {
		return Some(base);
	}
	if !offset.is_ascii() {
		return None;
	}
	let (sign, magnitude) = offset.split_at(1);
	let sign = match sign {
		"+" => 1i64,
		"-" => -1i64,
		_ => return None,
	};
	let (hours, minutes) = match magnitude.len() {
		// ISO 8601 offsets use a colon separator (e.g. +05:00); reject any other character.
		5 if magnitude.as_bytes()[2] == b':' => (&magnitude[..2], &magnitude[3..]),
		4 => (&magnitude[..2], &magnitude[2..]),
		_ => return None,
	};
	let hours: i64 = hours.parse().ok()?;
	let minutes: i64 = minutes.parse().ok()?;
	if hours > 23 || minutes > 59 {
		return None;
	}
	Some(base - sign * (hours * 3600 + minutes * 60))
}

pub fn valid_slug(value: &str) -> bool {
	!value.is_empty()
		&& value
			.chars()
			.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn valid_number(value: &str) -> bool {
	let mut has_dot = false;
	let mut digits = 0;
	let mut fractional_digits = 0;
	for byte in value.bytes() {
		match byte {
			b'0'..=b'9' => {
				digits += 1;
				if has_dot {
					fractional_digits += 1;
				}
			}
			b'.' if !has_dot && digits > 0 => has_dot = true,
			_ => return false,
		}
	}
	if digits == 0 || (has_dot && fractional_digits == 0) {
		return false;
	}
	match value.parse::<f32>() {
		Ok(number) => number.is_finite() && number > 0.0,
		Err(_) => false,
	}
}

pub fn body_to_text(body: String) -> Result<String> {
	let text = body
		.lines()
		.map(str::trim)
		.filter(|line| !line.is_empty())
		.map(|line| line.escape_markdown())
		.collect::<Vec<_>>()
		.join("\n\n");
	if text.is_empty() {
		bail!("Chikari returned an empty chapter")
	}
	Ok(text)
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku::alloc::vec;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn mature_rating_signals_are_suggestive() {
		let genres = vec![
			crate::models::Named {
				name: "Mature".into(),
				role: None,
			},
			crate::models::Named {
				name: "R-18".into(),
				role: None,
			},
		];
		assert_eq!(
			content_rating(false, &genres, &[]),
			ContentRating::Suggestive
		);
		assert_eq!(content_rating(true, &[], &[]), ContentRating::NSFW);
	}

	#[aidoku_test]
	fn parses_chikari_timestamp() {
		assert!(parse_iso_date("2026-02-21T22:08:14.600092+00:00").is_some());
	}

	#[aidoku_test]
	fn parses_timezone_offset_timestamp() {
		// A non-UTC offset must be normalized to UTC, not truncated and treated as UTC.
		let utc = parse_iso_date("2026-02-21T22:08:14+00:00").unwrap();
		let minus_five = parse_iso_date("2026-02-21T22:08:14-05:00").unwrap();
		assert_eq!(minus_five, utc + 5 * 3600);
	}

	#[aidoku_test]
	fn rejects_invalid_timezone_separator() {
		// A five-character offset must use a colon separator (e.g. -05:00), not any character.
		assert!(parse_iso_date("2026-02-21T22:08:14-05x00").is_none());
	}

	#[aidoku_test]
	fn list_item_rating_uses_nsfw_only() {
		let item = |is_nsfw| crate::models::NovelListItem {
			slug: "test".into(),
			title: "Test".into(),
			cover_url: None,
			status: None,
			is_nsfw,
		};
		assert_eq!(
			manga_from_list(item(false), ContentType::Novel).content_rating,
			ContentRating::Unknown
		);
		assert_eq!(
			manga_from_list(item(true), ContentType::Novel).content_rating,
			ContentRating::NSFW
		);
	}

	#[aidoku_test]
	fn list_url_retains_repeated_filters() {
		let filters = [FilterValue::MultiSelect {
			id: "genres".into(),
			included: vec!["action".into(), "fantasy".into()],
			excluded: vec!["mature".into(), "yuri".into()],
		}];
		let url = list_url(
			ContentType::Novel,
			"popular",
			36,
			0,
			Some("shadow slave"),
			&filters,
		);
		assert!(url.contains("q=shadow"));
		assert_eq!(url.matches("genre=").count(), 2);
		assert_eq!(url.matches("genre_exclude=").count(), 2);
		assert!(url.contains("genre=action"));
		assert!(url.contains("genre=fantasy"));
		assert!(url.contains("genre_exclude=mature"));
		assert!(url.contains("genre_exclude=yuri"));
	}

	#[aidoku_test]
	fn content_type_setting_maps_supported_values() {
		assert_eq!(
			content_type_from_setting_value(Some("novels")),
			ContentType::Novel
		);
		assert_eq!(
			content_type_from_setting_value(Some("comics")),
			ContentType::Series
		);
		assert_eq!(
			content_type_from_setting_value(Some("series")),
			ContentType::Series
		);
		assert_eq!(
			content_type_from_setting_value(Some("unexpected")),
			ContentType::Novel
		);
		assert_eq!(content_type_from_setting_value(None), ContentType::Novel);
	}

	#[aidoku_test]
	fn content_type_from_url() {
		assert_eq!(
			ContentType::from_url("https://chikari.moe/series/x"),
			ContentType::Series
		);
		assert_eq!(
			ContentType::from_url("https://chikari.moe/novels/x"),
			ContentType::Novel
		);
		assert_eq!(ContentType::from_url(""), ContentType::Novel);
	}

	#[aidoku_test]
	fn decodes_content_type_and_raw_slug() {
		assert_eq!(
			decode_manga_key("series:comic", None),
			(ContentType::Series, "comic")
		);
		assert_eq!(
			decode_manga_key("comic", Some("https://chikari.moe/series/comic")),
			(ContentType::Series, "comic")
		);
		assert_eq!(
			decode_manga_key("novel", None),
			(ContentType::Novel, "novel")
		);
	}

	#[aidoku_test]
	fn effective_limit_uses_fallback_only_for_zero() {
		assert_eq!(effective_limit(0, 60), 60);
		assert_eq!(effective_limit(36, 60), 36);
		assert!(!has_next(40, 0, 0, effective_limit(0, 60)));
		assert!(has_next(40, 0, 0, effective_limit(36, 60)));
	}

	#[aidoku_test]
	fn canonical_urls_and_series_keys() {
		let item = |slug: &str| crate::models::NovelListItem {
			slug: slug.into(),
			title: "Title".into(),
			cover_url: None,
			status: None,
			is_nsfw: false,
		};
		let novel = manga_from_list(item("novel"), ContentType::Novel);
		assert_eq!(novel.key, "novel");
		assert_eq!(
			novel.url.as_deref(),
			Some("https://chikari.moe/novels/novel")
		);
		let series = manga_from_list(item("series"), ContentType::Series);
		assert_eq!(series.key, "series:series");
		assert_eq!(
			series.url.as_deref(),
			Some("https://chikari.moe/series/series")
		);

		let detail = || crate::models::NovelDetail {
			slug: "detail".into(),
			title: "Title".into(),
			cover_url: None,
			description: None,
			status: None,
			is_nsfw: false,
			authors: vec![],
			reading_mode: None,
			kind: None,
			genres: vec![],
			tags: vec![],
		};
		let novel_detail = manga_from_detail(detail(), ContentType::Novel);
		assert_eq!(novel_detail.key, "detail");
		assert_eq!(
			novel_detail.url.as_deref(),
			Some("https://chikari.moe/novels/detail")
		);
		let series_detail = manga_from_detail(detail(), ContentType::Series);
		assert_eq!(series_detail.key, "series:detail");
		assert_eq!(
			series_detail.url.as_deref(),
			Some("https://chikari.moe/series/detail")
		);
		let converted_novel = manga_from_detail(detail(), ContentType::Novel);
		assert_eq!(converted_novel.key, "detail");
		assert_eq!(
			converted_novel.url.as_deref(),
			Some("https://chikari.moe/novels/detail")
		);
	}

	#[aidoku_test]
	fn empty_chapter_titles_are_none() {
		let empty = chapter_from_item(crate::models::ChapterItem {
			number: 1.0,
			title: Some("  ".into()),
			created_at: None,
		});
		assert!(empty.title.is_none());

		let missing = chapter_from_item(crate::models::ChapterItem {
			number: 2.0,
			title: None,
			created_at: None,
		});
		assert!(missing.title.is_none());

		let present = chapter_from_item(crate::models::ChapterItem {
			number: 3.0,
			title: Some("Chapter One".into()),
			created_at: None,
		});
		assert_eq!(present.title.as_deref(), Some("Chapter One"));
	}

	#[aidoku_test]
	fn maps_series_roles_and_reading_mode() {
		let manga = manga_from_detail(
			crate::models::NovelDetail {
				slug: "series".into(),
				title: "Title".into(),
				cover_url: None,
				description: None,
				status: None,
				is_nsfw: false,
				authors: vec![
					crate::models::Named {
						name: "Writer".into(),
						role: Some("author".into()),
					},
					crate::models::Named {
						name: "Artist".into(),
						role: Some("artist".into()),
					},
					crate::models::Named {
						name: "Artist".into(),
						role: Some("artist".into()),
					},
				],
				reading_mode: Some("strip".into()),
				kind: Some("manhwa".into()),
				genres: vec![],
				tags: vec![],
			},
			ContentType::Series,
		);
		assert_eq!(manga.authors, Some(vec!["Writer".into()]));
		assert_eq!(manga.artists, Some(vec!["Artist".into()]));
		assert_eq!(manga.viewer, Viewer::Webtoon);
	}

	#[aidoku_test]
	fn rejects_non_ascii_timezone_offset() {
		assert!(parse_iso_date("2026-02-21T22:08:14\u{2014}05:00").is_none());
	}

	#[aidoku_test]
	fn body_to_text_escapes_markdown() {
		let result = body_to_text("***".into()).expect("body_to_text failed");
		assert_eq!(result, "\\*\\*\\*");
	}
}
