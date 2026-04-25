#![no_std]

use aidoku::{
	alloc::{String, Vec, format, rc::Rc, string::ToString},
	imports::{
		defaults::{DefaultValue, defaults_get, defaults_set},
		error::AidokuError,
		net::{Request, TimeUnit, set_rate_limit},
		std::current_date,
	},
	prelude::*,
	Chapter, FilterValue, ImageRequestProvider, Manga, MangaPageResult, Page, PageContent,
	PageContext, Result, Source,
};
use core::fmt::Write;

mod filters;
mod helpers;
mod models;
mod settings;
mod wvd;

use helpers::{build_chapter_name, parse_date, status_from_str};
use models::*;

const BASE_URL: &str = "https://kagane.org";
const API_URL: &str = "https://yuzuki.kagane.org";

const SOURCE_NUMBER_FORMATS: &[&str] = &[
	"Dark Horse Comics",
	"Flame Comics",
	"MangaDex",
	"Square Enix Manga",
];

const INTEGRITY_TOKEN_KEY: &str = "kagane_integrity_token";
const INTEGRITY_EXP_KEY: &str = "kagane_integrity_exp";

fn get_integrity_token() -> Result<String> {
	let now = current_date();
	let cached_exp = defaults_get::<String>(INTEGRITY_EXP_KEY)
		.and_then(|s| s.parse::<i64>().ok())
		.unwrap_or(0);

	if now < cached_exp {
		if let Some(token) = defaults_get::<String>(INTEGRITY_TOKEN_KEY) {
			if !token.is_empty() {
				return Ok(token);
			}
		}
	}

	let text = Request::post(&format!("{BASE_URL}/api/integrity"))?
		.header("Content-Type", "application/json")
		.header("Origin", BASE_URL)
		.header("Referer", &format!("{BASE_URL}/"))
		.body("{}")
		.string()?;
	let dto: IntegrityDto =
		serde_json::from_str(&text).map_err(|e| AidokuError::JsonParseError(Rc::new(e)))?;

	defaults_set(INTEGRITY_TOKEN_KEY, DefaultValue::String(dto.token.clone()));
	defaults_set(INTEGRITY_EXP_KEY, DefaultValue::String(format!("{}", dto.exp)));

	Ok(dto.token)
}

struct Kagane;

impl Source for Kagane {
	fn new() -> Self {
		set_rate_limit(2, 1, TimeUnit::Seconds);
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filter_values: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let default_ratings = settings::get_default_content_ratings();
		let (body, sort_param) =
			filters::build_search_body(query.as_deref(), &filter_values, &default_ratings);

		let mut url = format!("{API_URL}/api/v2/search/series?page={}&size=35", page - 1);
		if !sort_param.is_empty() {
			let _ = write!(url, "&sort={sort_param}");
		}

		let text = Request::post(&url)?
			.header("Content-Type", "application/json")
			.header("Origin", BASE_URL)
			.header("Referer", &format!("{BASE_URL}/"))
			.body(body)
			.string()?;
		let dto: SearchDto =
			serde_json::from_str(&text).map_err(|e| AidokuError::JsonParseError(Rc::new(e)))?;

		let entries = dto
			.content
			.into_iter()
			.map(|book| Manga {
				key: book.series_id.clone(),
				title: book.title,
				cover: book.cover_image_id.map(|id| format!("{API_URL}/api/v2/image/{id}")),
				url: Some(format!("{BASE_URL}/series/{}", book.series_id)),
				..Default::default()
			})
			.collect();

		Ok(MangaPageResult {
			entries,
			has_next_page: !dto.last,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{API_URL}/api/v2/series/{}", manga.key);
		let text = Request::get(&url)?
			.header("Origin", BASE_URL)
			.header("Referer", &format!("{BASE_URL}/"))
			.string()?;
		let dto: DetailsDto =
			serde_json::from_str(&text).map_err(|e| AidokuError::JsonParseError(Rc::new(e)))?;

		if needs_details {
			let mut title = dto.title.trim().to_string();
			if settings::get_show_edition() {
				if let Some(ed) = dto.edition_info.as_deref().filter(|s| !s.is_empty()) {
					let _ = write!(title, " ({ed})");
				}
			}
			if settings::get_show_source() {
				if let Some(src) = dto.source_id.as_deref().filter(|s| !s.is_empty()) {
					let _ = write!(title, " [{src}]");
				}
			}
			manga.title = title;
			manga.status = status_from_str(&dto.upload_status);

			let authors: Vec<String> = dto
				.series_staff
				.iter()
				.filter(|s| {
					let role = s.role.to_ascii_lowercase();
					role.contains("author") || role.contains("story")
				})
				.map(|s| s.name.clone())
				.collect();
			if !authors.is_empty() {
				manga.authors = Some(authors);
			}

			let artists: Vec<String> = dto
				.series_staff
				.iter()
				.filter(|s| {
					let role = s.role.to_ascii_lowercase();
					role.contains("artist") || role.contains("art")
				})
				.map(|s| s.name.clone())
				.collect();
			if !artists.is_empty() {
				manga.artists = Some(artists);
			}

			let mut tags: Vec<String> = Vec::new();
			if let Some(fmt) = dto.format.as_deref().filter(|s| !s.is_empty()) {
				tags.push(fmt.to_string());
			}
			tags.extend(dto.genres.iter().map(|g| g.genre_name.clone()));
			if !tags.is_empty() {
				manga.tags = Some(tags);
			}

			let mut desc = String::new();
			if let Some(d) = dto.description.as_deref().filter(|s| !s.is_empty()) {
				desc.push_str(d.trim());
			}
			if !dto.series_alternate_titles.is_empty() {
				if !desc.is_empty() {
					desc.push_str("\n\n");
				}
				desc.push_str("Associated Names:\n");
				for alt in &dto.series_alternate_titles {
					let _ = writeln!(desc, "- {}", alt.title);
				}
			}
			if !desc.is_empty() {
				manga.description = Some(desc);
			}
		}

		if needs_chapters {
			let use_source_number = dto
				.format
				.as_deref()
				.map(|f| SOURCE_NUMBER_FORMATS.contains(&f))
				.unwrap_or(false);
			let mode = settings::get_chapter_title_mode();

			let chapters: Vec<Chapter> = dto
				.series_books
				.iter()
				.rev()
				.map(|book| {
					let key = format!("/series/{}/reader/{}", manga.key, book.book_id);
					let title = build_chapter_name(
						&book.title,
						book.chapter_no.as_deref(),
						book.volume_no.as_deref(),
						&mode,
					);
					let scanlators: Vec<String> =
						book.groups.iter().map(|g| g.title.clone()).collect();

					Chapter {
						key: key.clone(),
						title: if title.is_empty() { None } else { Some(title) },
						chapter_number: if use_source_number {
							Some(book.sort_no)
						} else {
							book.chapter_no.as_deref().and_then(|ch| ch.parse::<f32>().ok())
						},
						volume_number: book
							.volume_no
							.as_deref()
							.and_then(|vol| vol.parse::<f32>().ok()),
						date_uploaded: book.created_at.as_deref().map(|d| parse_date(d) as i64),
						scanlators: if scanlators.is_empty() {
							None
						} else {
							Some(scanlators)
						},
						url: Some(format!("{BASE_URL}{key}")),
						..Default::default()
					}
				})
				.collect();

			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let wvd_key = settings::get_wvd_key();
		if wvd_key.is_empty() {
			return Err(AidokuError::message(
				"WVD key required to read chapters. Add your WVD file (base64) in source settings.",
			));
		}

		let chapter_id = chapter
			.key
			.rsplit('/')
			.next()
			.filter(|s| !s.is_empty())
			.ok_or_else(|| AidokuError::message("Invalid chapter key format"))?;

		let integrity_token = get_integrity_token()?;
		let challenge = wvd::generate_challenge(&wvd_key, chapter_id)?;
		let data_saver = settings::get_data_saver();
		let challenge_url = format!("{API_URL}/api/v2/books/{chapter_id}?is_datasaver={data_saver}");
		let challenge_body = format!(r#"{{"challenge":"{challenge}"}}"#);

		let text = Request::post(&challenge_url)?
			.header("Content-Type", "application/json")
			.header("Origin", BASE_URL)
			.header("Referer", &format!("{BASE_URL}/"))
			.header("x-integrity-token", &integrity_token)
			.body(challenge_body)
			.string()?;
		let challenge_dto: ChallengeDto =
			serde_json::from_str(&text).map_err(|e| AidokuError::JsonParseError(Rc::new(e)))?;

		let cache_url = challenge_dto.cache_url;
		let access_token = challenge_dto.access_token;
		let mut pages = challenge_dto.pages;
		pages.sort_by_key(|p| p.page_number);

		Ok(pages
			.into_iter()
			.map(|page| {
				let url = format!(
					"{cache_url}/api/v2/books/file/{chapter_id}/{}?token={access_token}&is_datasaver={data_saver}",
					page.page_uuid
				);
				Page {
					content: PageContent::url(url),
					..Default::default()
				}
			})
			.collect())
	}
}

impl ImageRequestProvider for Kagane {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(&url)?
			.header("Origin", BASE_URL)
			.header("Referer", &format!("{BASE_URL}/")))
	}
}

register_source!(Kagane);

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn test_page_list_requires_wvd_key() {
		let source = Kagane::new();
		let result = source.get_page_list(
			Manga::default(),
			Chapter {
				key: "/series/series-id/reader/chapter-id".into(),
				..Default::default()
			},
		);
		assert!(result.is_err());
	}
}
