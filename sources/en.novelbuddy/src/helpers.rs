use aidoku::{
	AidokuError, Chapter, ContentRating, FilterValue, Manga, MangaPageResult, MangaStatus, Result,
	alloc::{String, Vec, string::ToString},
	helpers::uri::QueryParameters,
	imports::{html::Html, net::Request, std::parse_date},
	prelude::*,
};
use serde::de::DeserializeOwned;

use crate::models::{
	ApiResponse, BySlugData, ChapterListData, ChapterListItem, Genre, GenreData, ListData,
	TitleDetail, TitleListItem,
};
use crate::{API_URL, BASE_URL, USER_AGENT};

// Length separates the id from the slug in `/titles/<id>-<slug>`; relaxing it
// parses `/titles/shadow-slave` as the id `shadow`.
const CANONICAL_ID_LEN: usize = 8;

const ALLOWED_HOSTS: &[&str] = &["novelbuddy.me", "www.novelbuddy.me"];

pub fn request<T: DeserializeOwned>(url: &str) -> Result<T> {
	let response = Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.header("Accept", "application/json, text/plain, */*")
		.header("Referer", "https://novelbuddy.me/")
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

pub fn keeps_genres(item: &TitleListItem, hidden: &[String]) -> bool {
	// A trending entry carries the slugs in one string, a search entry carries them as objects.
	let in_slugs = item.genre_slugs.as_deref().is_some_and(|slugs| {
		slugs
			.split(',')
			.any(|slug| hidden.iter().any(|hidden| hidden == slug.trim()))
	});
	let in_genres = item.genres.iter().any(|genre| {
		genre
			.slug
			.as_deref()
			.is_some_and(|slug| hidden.iter().any(|hidden| hidden == slug))
	});
	!(in_slugs || in_genres)
}

pub fn keeps_status(item: &TitleListItem, statuses: &[String]) -> bool {
	if statuses.is_empty() {
		return true;
	}
	item.status.as_deref().is_some_and(|status| {
		statuses
			.iter()
			.any(|wanted| wanted.eq_ignore_ascii_case(status))
	})
}

/// A listing carries no filters of its own, so the saved selections are what
/// decides which entries it keeps.
pub fn listing_entries(
	items: Vec<TitleListItem>,
	statuses: &[String],
	ratings: &[String],
) -> Vec<Manga> {
	let hidden = crate::settings::hidden_genres();
	let mut entries: Vec<Manga> = items
		.into_iter()
		.filter(|item| keeps_genres(item, &hidden))
		.filter(|item| keeps_status(item, statuses))
		.map(Manga::from)
		.collect();
	if crate::settings::hide_mtl() {
		crate::mtl::retain_manga(&mut entries);
	}
	filter_by_rating(entries, ratings)
}

pub fn genres() -> Result<Vec<Genre>> {
	let url = format!("{API_URL}/genres");
	let data: GenreData = request(&url)?;
	Ok(data.items)
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
		let mut genre_names: Vec<String> = item.genres.into_iter().map(|g| g.name).collect();
		if let Some(slugs) = item.genre_slugs {
			for slug in slugs.split(',').map(str::trim).filter(|s| !s.is_empty()) {
				genre_names.push(slug.into());
			}
		}
		let content_rating = content_rating(item.is_adult, &genre_names);
		let status = item
			.status
			.as_deref()
			.map(parse_status)
			.unwrap_or(MangaStatus::Unknown);
		Manga {
			key: item.id,
			title: item.name,
			cover: item.cover,
			url,
			content_rating,
			status,
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

fn content_rating(is_adult: bool, tags: &[String]) -> ContentRating {
	if is_adult {
		return ContentRating::NSFW;
	}
	if tags.iter().any(|tag| is_suggestive_signal(tag)) {
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	}
}

fn is_suggestive_signal(value: &str) -> bool {
	const SUGGESTIVE_TAGS: &[&str] = &["ecchi", "smut", "yaoi", "yuri"];
	SUGGESTIVE_TAGS
		.iter()
		.any(|tag| value.trim().eq_ignore_ascii_case(tag))
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

pub fn absolute_url(path_or_url: &str) -> String {
	if path_or_url.starts_with("http") {
		path_or_url.into()
	} else if path_or_url.starts_with('/') {
		format!("{BASE_URL}{path_or_url}")
	} else {
		format!("{BASE_URL}/{path_or_url}")
	}
}

pub fn series_path(url: &str) -> Option<&str> {
	let without_fragment = url.split(['?', '#']).next()?;
	let (_, authority_and_path) = without_fragment.split_once("://")?;
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

pub fn parse_iso_date(value: &str) -> Option<i64> {
	// Fallback: strip fractional seconds and zone, then parse naively as UTC.
	parse_date(value, "yyyy-MM-dd'T'HH:mm:ss.SSS'Z'")
		.or_else(|| parse_date(value, "yyyy-MM-dd'T'HH:mm:ss'Z'"))
		.or_else(|| {
			let naive = value.split('.').next()?.trim_end_matches('Z');
			parse_date(naive, "yyyy-MM-ddTHH:mm:ss")
		})
}

pub fn parse_chapter_number(name: &str) -> Option<f32> {
	// Number is the first numeric run; keep a single '.' for decimal chapters.
	let mut num = String::default();
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
	if id.len() == CANONICAL_ID_LEN && id.chars().all(|c| c.is_ascii_alphanumeric()) {
		Some(id.into())
	} else {
		None
	}
}

pub fn html_to_text(html: &str) -> String {
	let Ok(doc) = Html::parse_fragment(html) else {
		return String::default();
	};
	let paragraphs = doc
		.select("p")
		.map(|els| {
			els.filter_map(|el| el.text())
				.map(|text| text.trim().to_string())
				.filter(|trimmed| !trimmed.is_empty())
				.collect::<Vec<_>>()
		})
		.unwrap_or_default();
	if !paragraphs.is_empty() {
		return paragraphs.join("\n\n");
	}
	// `select_first`, not `select`: a nested wrapper would concatenate its text
	// twice. `*` takes the outermost wrapper, whatever the tag is.
	doc.select_first("*")
		.and_then(|el| el.text())
		.map(|text| text.trim().to_string())
		.filter(|text| !text.is_empty())
		.unwrap_or_default()
}

pub const CONTENT_RATING_IDS: &[&str] = &["safe", "suggestive", "nsfw"];

const SORT_IDS: &[&str] = &[
	"popular",
	"latest",
	"newest",
	"bookmarks",
	"rating",
	"views_today",
	"views_7days",
	"views_30days",
	"views",
	"chapters",
];

pub const STATUS_IDS: &[&str] = &["ongoing", "completed", "hiatus", "cancelled"];

const SERVER_STATUS_IDS: &[&str] = &["ongoing", "completed"];

pub struct SearchContext {
	pub qs: QueryParameters,
	pub status_filter: Option<Vec<String>>,
	pub content_rating_filter: Option<Vec<String>>,
}

pub fn resolve_multi_select(
	all_options: &[&'static str],
	included: Vec<String>,
	excluded: Vec<String>,
) -> Vec<String> {
	let lower_excluded: Vec<String> = excluded.iter().map(|e| e.to_lowercase()).collect();
	let base: Vec<String> = if included.is_empty() {
		all_options.iter().map(|s| (*s).into()).collect()
	} else {
		included
	};
	base.into_iter()
		.filter(|s| !lower_excluded.contains(&s.to_lowercase()))
		.collect()
}

pub fn parse_filters(filters: Vec<FilterValue>) -> SearchContext {
	let mut qs = QueryParameters::new();
	let mut has_sort = false;
	let mut excluded_genres = crate::settings::hidden_genres();
	let mut status_filter: Option<Vec<String>> = None;
	let mut content_rating_filter: Option<Vec<String>> = None;

	for filter in filters {
		match filter {
			FilterValue::Sort { index, .. } => {
				if let Some(s) = SORT_IDS.get(index as usize) {
					qs.push("sort", Some(*s));
					has_sort = true;
				}
			}
			FilterValue::MultiSelect {
				id,
				included,
				excluded,
			} if id == "status" => {
				status_filter = Some(resolve_multi_select(STATUS_IDS, included, excluded));
			}
			FilterValue::MultiSelect {
				id,
				included,
				excluded,
			} if id == "contentRating" => {
				content_rating_filter =
					Some(resolve_multi_select(CONTENT_RATING_IDS, included, excluded));
			}
			FilterValue::MultiSelect {
				id,
				included,
				excluded,
			} if id == "genres" => {
				for genre in &included {
					if let Some(pos) = excluded_genres.iter().position(|g| g == genre) {
						excluded_genres.swap_remove(pos);
					}
				}
				if !included.is_empty() {
					qs.push("genres", Some(&included.join(",")));
				}
				for genre in excluded {
					if !excluded_genres.contains(&genre) {
						excluded_genres.push(genre);
					}
				}
			}
			_ => {}
		}
	}

	if !has_sort {
		qs.push("sort", Some("popular"));
	}
	if !excluded_genres.is_empty() {
		qs.push("exclude", Some(&excluded_genres.join(",")));
	}

	SearchContext {
		qs,
		status_filter,
		content_rating_filter,
	}
}

pub fn build_search_url(
	ctx: &SearchContext,
	query: Option<&str>,
	page: i32,
	status: Option<&str>,
) -> String {
	let mut qs = ctx.qs.clone();
	if query.is_some() {
		qs.push("q", query);
	}
	if status.is_some() {
		qs.push("status", status);
	}
	let page = page.max(1).to_string();
	qs.push("page", Some(&page));
	format!("{API_URL}/titles/search?{qs}")
}

/// The saved status and content rating act as defaults: they fill a search that
/// carries neither, and they apply to listings, which carry no filters at all.
pub fn apply_settings(filters: Vec<FilterValue>) -> Vec<FilterValue> {
	let selected_status = crate::settings::status_filter();
	let selected_ratings = crate::settings::content_rating_filter();
	let mut filters = filters;
	push_persistent_selection(&mut filters, "status", selected_status);
	push_persistent_selection(&mut filters, "contentRating", selected_ratings);
	filters
}

fn push_persistent_selection(filters: &mut Vec<FilterValue>, id: &str, selected: Vec<String>) {
	if selected.is_empty() || has_filter(filters, id) {
		return;
	}
	filters.push(FilterValue::MultiSelect {
		id: id.into(),
		included: selected,
		excluded: Vec::new(),
	});
}

fn has_filter(filters: &[FilterValue], id: &str) -> bool {
	filters
		.iter()
		.any(|filter| matches!(filter, FilterValue::MultiSelect { id: found, .. } if found == id))
}

pub fn is_server_status(value: &str) -> bool {
	SERVER_STATUS_IDS
		.iter()
		.any(|supported| value.eq_ignore_ascii_case(supported))
}

fn rating_matches(rating: &str, content_rating: aidoku::ContentRating) -> bool {
	matches!(
		(rating, content_rating),
		("safe", aidoku::ContentRating::Safe)
			| ("suggestive", aidoku::ContentRating::Suggestive)
			| ("nsfw", aidoku::ContentRating::NSFW)
	)
}

pub fn filter_by_rating(entries: Vec<Manga>, ratings: &[String]) -> Vec<Manga> {
	if ratings.is_empty() {
		return entries;
	}
	entries
		.into_iter()
		.filter(|m| ratings.iter().any(|r| rating_matches(r, m.content_rating)))
		.collect()
}

pub fn fetch_multi_status(
	ctx: &SearchContext,
	query: Option<&str>,
	page: i32,
	all_statuses: &[String],
	all_ratings: &[String],
	hide: bool,
) -> Result<MangaPageResult> {
	let mut all_entries: Vec<Manga> = Vec::new();
	let mut any_has_next = false;
	let mut first_error: Option<AidokuError> = None;
	let mut success_count = 0usize;

	for status in all_statuses {
		let url = build_search_url(ctx, query, page, Some(status));
		match request::<ListData>(&url) {
			Ok(list_data) => {
				let mut filtered: Vec<Manga> = list_data
					.items
					.into_iter()
					.filter(|item| {
						item.status
							.as_deref()
							.is_some_and(|s| s.eq_ignore_ascii_case(status))
					})
					.map(Manga::from)
					.collect();
				if list_data.pagination.has_next {
					any_has_next = true;
				}
				all_entries.append(&mut filtered);
				success_count += 1;
			}
			Err(e) => {
				if first_error.is_none() {
					first_error = Some(e);
				}
			}
		}
	}

	if success_count == 0 {
		if let Some(e) = first_error {
			return Err(e);
		}
		bail!("all status requests failed");
	}

	if hide {
		crate::mtl::retain_manga(&mut all_entries);
	}
	let mut entries = filter_by_rating(all_entries, all_ratings);
	let mut seen: Vec<String> = Vec::new();
	entries.retain(|m| {
		if seen.contains(&m.key) {
			false
		} else {
			seen.push(m.key.clone());
			true
		}
	});
	Ok(MangaPageResult {
		entries,
		has_next_page: any_has_next,
	})
}

pub fn deep_link_slugs(path: &str) -> (&str, &str) {
	let mut segments = path.split('/').filter(|s| !s.is_empty());
	let series = segments.next().unwrap_or("");
	let chapter = segments.next().unwrap_or("");
	(series, chapter)
}

pub fn chapter_url_matches(url: &str, chapter_slug: &str) -> bool {
	!chapter_slug.is_empty() && url.trim_end_matches('/').rsplit('/').next() == Some(chapter_slug)
}
