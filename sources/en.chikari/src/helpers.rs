use aidoku::{
	Chapter, ContentRating, DeepLinkResult, FilterValue, Manga, MangaPageResult, MangaStatus, Page,
	PageContent, Result, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::{net::Request, std::parse_date},
	prelude::*,
};
use serde::de::DeserializeOwned;

use crate::models::{
	ChapterBody, ChapterItem, ChapterListResponse, Detail, Genre, ListItem, ListResponse,
};
use crate::{BASE_URL, USER_AGENT};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ContentType {
	Novel,
	Series,
}

pub const SERIES_KEY_PREFIX: &str = "series:";

// The API hides novels unless a type is given, so every request sends all of them.
pub(crate) const TYPES: &[&str] = &["manga", "manhwa", "manhua", "oel", "novel"];

// The API hides erotica and pornographic tiers unless requested, so browse sends every tier.
pub(crate) const CONTENT_RATINGS: &[&str] = &["safe", "suggestive", "erotica", "pornographic"];

pub(crate) const STATUS: &[&str] = &["releasing", "completed", "hiatus", "cancelled"];

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

pub fn genres() -> Result<Vec<Genre>> {
	request(&format!("{BASE_URL}/api/genres"))
}

pub fn request<T: DeserializeOwned>(url: &str) -> Result<T> {
	// A refusal comes back as an ordinary body, so the required fields of each
	// response are what tells a real answer from one.
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
	let (status_in, status_ex) = multi_select(filters, "status");
	let (ratings_in, ratings_ex) = multi_select(filters, "contentRating");
	let (Some(types), Some(ratings)) = (
		effective(TYPES, types_in, types_ex),
		effective(CONTENT_RATINGS, ratings_in, ratings_ex),
	) else {
		return None;
	};
	for kind in types {
		qs.push("type", Some(kind));
	}
	if !status_in.is_empty() || !status_ex.is_empty() {
		for status in effective(STATUS, status_in, status_ex)? {
			qs.push("status", Some(status));
		}
	}
	qs.push("content_rating", Some(&ratings.join(",")));
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

/// An empty selection means every tier, which pins nothing.
pub fn selected_ratings(filters: &[FilterValue]) -> Vec<&'static str> {
	let (included, excluded) = multi_select(filters, "contentRating");
	effective(CONTENT_RATINGS, included, excluded).unwrap_or_default()
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

pub fn manga_page_result(
	data: ListResponse,
	offset: u32,
	limit: u32,
	pinned: Option<ContentRating>,
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
			.map(|item| manga_from_list(item, pinned))
			.collect(),
		has_next_page,
	}
}

/// When the request pins a single tier the server returns nothing else, so every
/// entry carries that tier instead of falling back to the `is_nsfw` binary.
pub fn pinned_rating(ratings: &[&str]) -> Option<ContentRating> {
	match ratings {
		["safe"] => Some(ContentRating::Safe),
		["suggestive"] => Some(ContentRating::Suggestive),
		["erotica"] | ["pornographic"] => Some(ContentRating::NSFW),
		_ => None,
	}
}

pub fn manga_from_list(item: ListItem, pinned: Option<ContentRating>) -> Manga {
	let content_type = ContentType::from_list_item(item.medium.as_deref(), item.kind.as_deref());
	let slug = item.slug;
	let key = deep_link_manga_key(content_type, &slug);
	let url = Some(format!("{BASE_URL}{}/{slug}", content_type.web_base()));
	Manga {
		key,
		title: item.title,
		cover: item.cover_url,
		url,
		content_rating: pinned.unwrap_or_else(|| content_rating(item.is_nsfw)),
		status: item.status.as_deref().map(parse_status).unwrap_or_default(),
		..Default::default()
	}
}

pub fn manga_from_detail(detail: Detail, content_type: ContentType) -> Manga {
	let slug = detail.slug;
	let key = deep_link_manga_key(content_type, &slug);
	let url = Some(format!("{BASE_URL}{}/{slug}", content_type.web_base()));
	let content_rating = detail_rating(
		detail.is_nsfw,
		rated_suggestive(&detail.title, &slug, content_type).ok(),
	);
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

pub fn content_rating(is_nsfw: bool) -> ContentRating {
	if is_nsfw {
		ContentRating::NSFW
	} else {
		ContentRating::Unknown
	}
}

/// `is_nsfw` cannot separate safe from suggestive, but the server can: a title
/// appears under `content_rating=suggestive` only when the site rates it so.
/// `q=` only searches within the types it is given, and the API hides novels
/// unless `type` is present, so the lookup sends every type.
pub fn suggestive_lookup_url(title: &str) -> String {
	let mut qs = QueryParameters::new();
	qs.push("q", Some(title));
	qs.push("content_rating", Some("suggestive"));
	qs.push("limit", Some("30"));
	for kind in TYPES {
		qs.push("type", Some(kind));
	}
	format!("{BASE_URL}/api/series?{qs}")
}

pub fn rated_suggestive(title: &str, slug: &str, content_type: ContentType) -> Result<bool> {
	let data: ListResponse = request(&suggestive_lookup_url(title))?;
	Ok(data.items.iter().any(|item| {
		item.slug == slug
			&& ContentType::from_list_item(item.medium.as_deref(), item.kind.as_deref())
				== content_type
	}))
}

/// `None` means the lookup failed and establishes nothing, so the rating stays
/// `Unknown` rather than claiming `Safe`.
pub fn detail_rating(is_nsfw: bool, suggestive: Option<bool>) -> ContentRating {
	if is_nsfw {
		return ContentRating::NSFW;
	}
	match suggestive {
		Some(true) => ContentRating::Suggestive,
		Some(false) => ContentRating::Safe,
		None => ContentRating::Unknown,
	}
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

pub fn chikari_path(url: &str) -> Option<&str> {
	let without_fragment = url.split(['?', '#']).next()?;
	let (_, authority_and_path) = without_fragment.split_once("://")?;
	let (_, path) = authority_and_path
		.split_once('/')
		.unwrap_or((authority_and_path, ""));
	Some(path)
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

pub fn apply_settings(filters: Vec<FilterValue>) -> Vec<FilterValue> {
	let hidden = crate::settings::hidden_genres();
	let selected_types = crate::settings::type_filters();
	let selected_status = crate::settings::status_filter();
	let selected_ratings = crate::settings::content_rating_filter();
	if hidden.is_empty()
		&& selected_types.is_empty()
		&& selected_status.is_empty()
		&& selected_ratings.is_empty()
	{
		return filters;
	}
	let mut filters = filters;
	if !hidden.is_empty() {
		if let Some(FilterValue::MultiSelect {
			included, excluded, ..
		}) = filters
			.iter_mut()
			.find(|filter| matches!(filter, FilterValue::MultiSelect { id, .. } if id == "genres"))
		{
			excluded.extend(hidden_genres_to_exclude(hidden, included));
		} else {
			filters.push(FilterValue::MultiSelect {
				id: "genres".into(),
				included: Vec::new(),
				excluded: hidden,
			});
		}
	}
	push_persistent_selection(&mut filters, "types", selected_types);
	push_persistent_selection(&mut filters, "status", selected_status);
	push_persistent_selection(&mut filters, "contentRating", selected_ratings);
	filters
}

fn push_persistent_selection(
	filters: &mut Vec<FilterValue>,
	id: &str,
	selected: Vec<&'static str>,
) {
	if selected.is_empty() || has_filter(filters, id) {
		return;
	}
	filters.push(FilterValue::MultiSelect {
		id: id.into(),
		included: selected.into_iter().map(Into::into).collect(),
		excluded: Vec::new(),
	});
}

fn has_filter(filters: &[FilterValue], id: &str) -> bool {
	filters
		.iter()
		.any(|filter| matches!(filter, FilterValue::MultiSelect { id: found, .. } if found == id))
}

pub fn novel_text_pages(data: ChapterBody) -> Result<Vec<Page>> {
	let text = if data.locked || data.body.trim().is_empty() {
		"This chapter is locked (early access)".into()
	} else {
		body_to_text(data.body)?
	};
	Ok(vec![Page {
		content: PageContent::text(text),
		..Default::default()
	}])
}

pub fn series_image_pages(pages: Vec<String>) -> Vec<Page> {
	pages
		.into_iter()
		.map(|url| Page {
			content: PageContent::url(url),
			..Default::default()
		})
		.collect()
}

pub fn chapter_deep_link(manga_key: String, number: &str) -> Option<DeepLinkResult> {
	let number: f32 = number.parse().ok()?;
	Some(DeepLinkResult::Chapter {
		manga_key,
		key: chapter_key(number),
	})
}
