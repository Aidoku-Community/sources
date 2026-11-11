use crate::models::{ChapterItem, ChapterListResponse, Detail, ListItem, ListResponse};
use crate::{BASE_URL, USER_AGENT};
use aidoku::{
	Chapter, ContentRating, FilterValue, Manga, MangaPageResult, MangaStatus, Result, Viewer,
	alloc::{String, Vec, string::ToString},
	helpers::uri::QueryParameters,
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

// The API hides novels unless a type is given, so browse sends every type.
const TYPES: &[&str] = &["manga", "manhwa", "manhua", "oel", "novel"];

// The API hides erotica and pornographic tiers unless ratings are given, so browse sends every tier.
const RATINGS: &[&str] = &["safe", "suggestive", "erotica", "pornographic"];

const STATUS: &[&str] = &["releasing", "completed", "hiatus", "cancelled", "upcoming"];

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

	pub fn from_list_item(medium: Option<&str>, kind: Option<&str>) -> Self {
		match medium {
			Some("novel") => Self::Novel,
			Some(_) => Self::Series,
			None if kind == Some("novel") => Self::Novel,
			None => Self::Series,
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

pub fn request<T: DeserializeOwned>(url: &str) -> Result<T> {
	Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.header("Accept", "application/json")
		.header("Referer", BASE_URL)
		.header("Origin", BASE_URL)
		.json_owned::<T>()
}

pub fn list_url(
	sort: &str,
	limit: u32,
	offset: u32,
	query: Option<&str>,
	filters: &[FilterValue],
) -> Option<String> {
	let mut qs = QueryParameters::new();
	qs.push("sort", Some(sort));
	qs.push("limit", Some(&limit.to_string()));
	qs.push("offset", Some(&offset.to_string()));
	if let Some(query) = query {
		qs.push("q", Some(query));
	}
	let (types_in, types_ex) = multi_select(filters, "types");
	let (ratings_in, ratings_ex) = multi_select(filters, "ratings");
	let (status_in, status_ex) = multi_select(filters, "status");
	let (Some(types), Some(ratings)) = (
		effective(TYPES, types_in, types_ex),
		effective(RATINGS, ratings_in, ratings_ex),
	) else {
		return None;
	};
	for kind in types {
		qs.push("type", Some(kind));
	}
	qs.push("content_rating", Some(&ratings.join(",")));
	if !status_in.is_empty() || !status_ex.is_empty() {
		for status in effective(STATUS, status_in, status_ex)? {
			qs.push("status", Some(status));
		}
	}
	for filter in filters {
		let FilterValue::MultiSelect {
			id,
			included,
			excluded,
		} = filter
		else {
			continue;
		};
		if id == "genres" {
			for genre in included {
				qs.push("genre", Some(genre));
			}
			for genre in excluded {
				qs.push("genre_exclude", Some(genre));
			}
		}
	}
	Some(format!("{BASE_URL}/api/series?{qs}"))
}

pub fn hidden_genres_to_exclude(hidden: Vec<String>, included: &[String]) -> Vec<String> {
	hidden
		.into_iter()
		.filter(|hidden_genre| {
			!included
				.iter()
				.any(|kept| kept.eq_ignore_ascii_case(hidden_genre))
		})
		.collect()
}

fn multi_select<'a>(filters: &'a [FilterValue], id: &str) -> (&'a [String], &'a [String]) {
	filters
		.iter()
		.find_map(|filter| match filter {
			FilterValue::MultiSelect {
				id: found,
				included,
				excluded,
			} if found == id => Some((included.as_slice(), excluded.as_slice())),
			_ => None,
		})
		.unwrap_or((&[], &[]))
}

fn effective(
	all: &[&'static str],
	included: &[String],
	excluded: &[String],
) -> Option<Vec<&'static str>> {
	let mut values: Vec<&'static str> = if included.is_empty() {
		all.to_vec()
	} else {
		included
			.iter()
			.filter_map(|value| {
				all.iter()
					.copied()
					.find(|known| value.eq_ignore_ascii_case(known))
			})
			.collect()
	};
	values.retain(|value| !excluded.iter().any(|e| e.eq_ignore_ascii_case(value)));
	(!values.is_empty()).then_some(values)
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

pub fn manga_page_result(data: ListResponse, offset: u32, limit: u32) -> MangaPageResult {
	let response_offset = effective_offset(data.offset, offset);
	let has_next_page = has_next(
		data.items.len(),
		data.total,
		response_offset,
		effective_limit(data.limit, limit),
	);
	MangaPageResult {
		entries: data.items.into_iter().map(manga_from_list).collect(),
		has_next_page,
	}
}

pub fn manga_from_list(item: ListItem) -> Manga {
	let content_type = ContentType::from_list_item(item.medium.as_deref(), item.kind.as_deref());
	let slug = item.slug;
	let key = deep_link_manga_key(content_type, &slug);
	let url = Some(format!("{BASE_URL}{}/{slug}", content_type.web_base()));
	Manga {
		key,
		title: item.title,
		cover: item.cover_url,
		url,
		content_rating: list_content_rating(item.is_nsfw),
		status: item.status.as_deref().map(parse_status).unwrap_or_default(),
		..Default::default()
	}
}

pub fn list_content_rating(is_nsfw: bool) -> ContentRating {
	if is_nsfw {
		ContentRating::NSFW
	} else {
		ContentRating::Safe
	}
}

pub fn manga_from_detail(detail: Detail, content_type: ContentType) -> Manga {
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
	let suggestive = genres
		.iter()
		.map(|genre| genre.name.as_str())
		.chain(tags.iter().map(|tag| tag.name.as_str()))
		.any(is_suggestive_signal);
	if suggestive {
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	}
}

fn is_suggestive_signal(value: &str) -> bool {
	const SUGGESTIVE_TAGS: &[&str] = &[
		"borderline h",
		"ecchi",
		"erotica",
		"lolicon",
		"mature",
		"nudity",
		"shotacon",
	];
	SUGGESTIVE_TAGS
		.iter()
		.any(|tag| value.trim().eq_ignore_ascii_case(tag))
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
	// Fallback for hosts that parse neither fractional seconds nor zones:
	// drop both and read the value as UTC.
	parse_date(value, "yyyy-MM-dd'T'HH:mm:ss.SSSSSSXXX").or_else(|| {
		let naive = value.split('.').next()?;
		let naive = naive.trim_end_matches("+00:00").trim_end_matches('Z');
		parse_date(naive, "yyyy-MM-ddTHH:mm:ss")
	})
}

const ALLOWED_HOSTS: &[&str] = &["chikari.moe", "www.chikari.moe"];
const ALLOWED_SCHEMES: &[&str] = &["http", "https"];

pub fn chikari_path(url: &str) -> Option<&str> {
	let without_fragment = url.split(['?', '#']).next()?;
	let (scheme, authority_and_path) = without_fragment.split_once("://")?;
	if !ALLOWED_SCHEMES.contains(&scheme.to_ascii_lowercase().as_str()) {
		return None;
	}
	let (authority, path) = authority_and_path
		.split_once('/')
		.unwrap_or((authority_and_path, ""));
	if authority.contains('\\') {
		return None;
	}
	let host = authority
		.rsplit('@')
		.next()
		.unwrap_or(authority)
		.split(':')
		.next()
		.unwrap_or(authority);
	let host = host.to_ascii_lowercase();
	ALLOWED_HOSTS.contains(&host.as_str()).then_some(path)
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
	let text = crate::markdown::to_markdown(&body);
	if text.is_empty() {
		bail!("Chikari returned an empty chapter");
	}
	Ok(text)
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku::alloc::vec;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn chikari_path_accepts_only_catalog_hosts() {
		assert_eq!(
			chikari_path("https://chikari.moe/novels/a?x=1"),
			Some("novels/a")
		);
		assert_eq!(
			chikari_path("http://www.chikari.moe/series/b/#frag"),
			Some("series/b/")
		);
		assert_eq!(
			chikari_path("https://CHIKARI.MOE:443/novels/c"),
			Some("novels/c")
		);
		assert_eq!(chikari_path("https://evil.com/chikari.moe/novels/a"), None);
		assert_eq!(chikari_path("https://chikari.moe.evil.com/novels/a"), None);
		assert_eq!(chikari_path("chikari.moe/novels/a"), None);
		assert_eq!(chikari_path("ftp://chikari.moe/novels/a"), None);
		assert_eq!(
			chikari_path(r"https://evil.com\@chikari.moe/novels/a"),
			None
		);
		assert_eq!(
			chikari_path("HTTPS://chikari.moe/novels/d"),
			Some("novels/d")
		);
	}

	#[aidoku_test]
	fn list_url_sends_types_and_repeated_genres() {
		let filters = [
			FilterValue::MultiSelect {
				id: "types".into(),
				included: vec!["manga".into(), "novel".into()],
				excluded: vec!["manga".into()],
			},
			FilterValue::MultiSelect {
				id: "genres".into(),
				included: vec!["action".into(), "fantasy".into()],
				excluded: vec!["mature".into()],
			},
		];
		let url = list_url("popular", 36, 0, Some("shadow slave"), &filters).unwrap();
		assert!(url.starts_with("https://chikari.moe/api/series?"));
		assert!(url.contains("type=novel"));
		assert!(!url.contains("type=manga"));
		assert_eq!(url.matches("genre=").count(), 2);
		assert!(url.contains("genre=action"));
		assert!(url.contains("genre_exclude=mature"));
		assert!(url.contains("q=shadow"));
	}

	#[aidoku_test]
	fn list_url_defaults_to_every_type_and_rejects_empty_selection() {
		let url = list_url("popular", 36, 0, None, &[]).unwrap();
		assert_eq!(url.matches("type=").count(), TYPES.len());
		let filters = [FilterValue::MultiSelect {
			id: "types".into(),
			included: vec![],
			excluded: vec!["novel".into()],
		}];
		let url = list_url("popular", 36, 0, None, &filters).unwrap();
		assert_eq!(url.matches("type=").count(), TYPES.len() - 1);
		assert!(!url.contains("type=novel"));
		let filters = [FilterValue::MultiSelect {
			id: "types".into(),
			included: vec![],
			excluded: TYPES.iter().map(|kind| (*kind).into()).collect(),
		}];
		assert!(list_url("popular", 36, 0, None, &filters).is_none());
	}

	#[aidoku_test]
	fn hidden_genres_skip_ones_the_user_included() {
		assert_eq!(
			hidden_genres_to_exclude(vec!["mature".into(), "smut".into()], &["Mature".into()]),
			vec!["smut"]
		);
		assert_eq!(
			hidden_genres_to_exclude(Vec::new(), &["action".into()]),
			Vec::<String>::new()
		);
	}

	#[aidoku_test]
	fn list_url_sends_content_ratings() {
		let filters = [FilterValue::MultiSelect {
			id: "ratings".into(),
			included: vec!["safe".into(), "erotica".into()],
			excluded: vec![],
		}];
		let url = list_url("popular", 36, 0, None, &filters).unwrap();
		assert!(url.contains("content_rating=safe"));
		assert!(url.contains("erotica"));
		assert!(!url.contains("pornographic"));
		let url = list_url("popular", 36, 0, None, &[]).unwrap();
		assert!(url.contains("content_rating=safe"));
		assert!(url.contains("pornographic"));
		let filters = [FilterValue::MultiSelect {
			id: "ratings".into(),
			included: vec![],
			excluded: RATINGS.iter().map(|rating| (*rating).into()).collect(),
		}];
		assert!(list_url("popular", 36, 0, None, &filters).is_none());
	}

	#[aidoku_test]
	fn list_url_sends_status_only_when_restricted() {
		let url = list_url("popular", 36, 0, None, &[]).unwrap();
		assert!(!url.contains("status="));
		let filters = [FilterValue::MultiSelect {
			id: "status".into(),
			included: vec!["completed".into(), "hiatus".into()],
			excluded: vec![],
		}];
		let url = list_url("popular", 36, 0, None, &filters).unwrap();
		assert!(url.contains("status=completed"));
		assert!(url.contains("status=hiatus"));
		assert!(!url.contains("status=releasing"));
		let filters = [FilterValue::MultiSelect {
			id: "status".into(),
			included: vec![],
			excluded: STATUS.iter().map(|status| (*status).into()).collect(),
		}];
		assert!(list_url("popular", 36, 0, None, &filters).is_none());
	}

	#[aidoku_test]
	fn key_and_url_follow_the_item_medium() {
		let item = |medium: Option<&str>, kind: Option<&str>| crate::models::ListItem {
			slug: "slug".into(),
			title: "Title".into(),
			cover_url: None,
			status: None,
			is_nsfw: true,
			medium: medium.map(Into::into),
			kind: kind.map(Into::into),
		};
		let novel = manga_from_list(item(Some("novel"), None));
		assert_eq!(novel.key, "slug");
		assert_eq!(
			novel.url.as_deref(),
			Some("https://chikari.moe/novels/slug")
		);
		assert_eq!(novel.content_rating, ContentRating::NSFW);
		let comic = manga_from_list(item(Some("comic"), None));
		assert_eq!(comic.key, "series:slug");
		assert_eq!(
			comic.url.as_deref(),
			Some("https://chikari.moe/series/slug")
		);
		let fallback = manga_from_list(item(None, Some("novel")));
		assert_eq!(fallback.key, "slug");
	}

	#[aidoku_test]
	fn manga_key_decoding_routes_the_pipeline() {
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
	fn parse_iso_date_accepts_chikari_timestamps() {
		let micros = parse_iso_date("2026-02-21T22:08:14.600092+00:00").unwrap();
		assert_eq!(
			micros,
			parse_iso_date("2026-02-21T22:08:14.000000+00:00").unwrap()
		);
		assert_eq!(
			parse_iso_date("2026-02-21T22:08:14.000000-05:00").unwrap(),
			parse_iso_date("2026-02-21T22:08:14").unwrap()
		);
		assert!(parse_iso_date("2026-02-21T22:08:14Z").is_some());
	}
}
