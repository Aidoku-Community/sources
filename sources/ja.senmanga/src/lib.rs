#![no_std]
use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Manga, MangaPageResult,
	MangaStatus, Page, PageContent, Result, Source, Viewer,
	alloc::{String, Vec, string::ToString},
	helpers::uri::QueryParameters,
	imports::{
		net::Request,
		std::{parse_date, send_partial_result},
	},
	prelude::*,
};

mod models;
#[cfg(test)]
mod test;

use models::*;

const BASE_URL: &str = "https://raw.senmanga.com";
const API_URL: &str = "https://raw.senmanga.com/api";

/// Order values accepted by `/api/directory`, in the same order as the sort
/// options in res/filters.json. Any other value makes the api return a 500.
const SORT_VALUES: [&str; 4] = ["popular", "title", "updated", "rating"];

/// Format of `chapterList[].datetime`, e.g. "2026-08-06T12:04:20Z". The offset
/// has to be read with `XXX` rather than a literal `'Z'`, or the timestamp gets
/// interpreted in the device timezone instead of utc.
const DATE_FORMAT: &str = "yyyy-MM-dd'T'HH:mm:ssXXX";

/// Tags that mark an entry as explicit.
const NSFW_TAGS: [&str; 6] = ["Adult", "Smut", "Lolicon", "Shotacon", "Yaoi", "Yuri"];

/// Tags that mark an entry as suggestive.
const SUGGESTIVE_TAGS: [&str; 2] = ["Ecchi", "Mature"];

struct SenManga;

impl Source for SenManga {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut params = QueryParameters::new();
		params.push("page", Some(&page.to_string()));

		if let Some(query) = query.as_deref() {
			params.push("query", Some(query));
		}

		for filter in filters {
			match filter {
				FilterValue::Sort { index, .. } => {
					if let Some(value) = SORT_VALUES.get(index as usize) {
						params.push("order", Some(value));
					}
				}
				// An empty value is the "Any" option, which the api rejects.
				FilterValue::Select { id, value } if !value.is_empty() => {
					params.push(&id, Some(&value));
				}
				_ => {}
			}
		}

		let url = format!("{API_URL}/directory?{params}");
		let DirectoryResponse {
			current_page,
			total_pages,
			series,
		} = Request::get(&url)?
			.send()?
			.get_json::<DirectoryResponse>()?;

		let entries = series
			.into_iter()
			.map(|entry| {
				let SeriesEntry {
					title,
					slug,
					cover,
					status,
				} = entry;
				Manga {
					url: Some(format!("{BASE_URL}/manga/{slug}/")),
					key: slug,
					title,
					cover,
					status: parse_status(status.as_deref()),
					..Default::default()
				}
			})
			.collect::<Vec<Manga>>();

		// Both fields are null when the result fits on a single page.
		let has_next_page = match (current_page, total_pages) {
			(Some(current), Some(total)) => current < total,
			_ => false,
		};

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{API_URL}/manga/{}", manga.key);
		let MangaDetails {
			title,
			cover,
			genre,
			kind,
			status,
			description,
			chapter_list,
		} = Request::get(&url)?.send()?.get_json::<MangaDetails>()?;

		if needs_details {
			let tags = genre
				.map(|genre| {
					genre
						.split(',')
						.map(|tag| tag.trim())
						.filter(|tag| !tag.is_empty())
						.map(String::from)
						.collect::<Vec<String>>()
				})
				.unwrap_or_default();

			manga.content_rating = content_rating(&tags);
			manga.viewer = match kind.as_deref() {
				Some("Manhwa") | Some("Manhua") => Viewer::Webtoon,
				_ => Viewer::RightToLeft,
			};
			// This endpoint answers with a null status, while the directory
			// entry the manga came from carries one, so the existing value is
			// kept rather than overwritten with Unknown.
			if let Some(status) = status {
				manga.status = parse_status(Some(&status));
			}
			manga.url = Some(format!("{BASE_URL}/manga/{}/", manga.key));
			manga.title = title;
			manga.cover = cover;
			manga.description = description;
			manga.tags = Some(tags);

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			manga.chapters = Some(
				chapter_list
					.into_iter()
					.map(|entry| {
						let ChapterEntry {
							title,
							number,
							url,
							full_url,
							datetime,
						} = entry;
						let chapter_number =
							number.as_deref().and_then(|it| it.parse::<f32>().ok());
						// Most titles just repeat the number ("Chapter 8"), which
						// the app already displays on its own.
						let title = title.filter(|title| {
							!matches!(
								number.as_deref(),
								Some(number)
									if title.trim_start_matches("Chapter").trim() == number
							)
						});
						Chapter {
							key: url,
							title,
							chapter_number,
							date_uploaded: datetime.and_then(|it| parse_date(it, DATE_FORMAT)),
							url: full_url.map(|path| format!("{BASE_URL}{path}")),
							..Default::default()
						}
					})
					.collect::<Vec<Chapter>>(),
			);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{API_URL}/read/{}/{}", manga.key, chapter.key);
		let ReadResponse { pages } = Request::get(&url)?.send()?.get_json::<ReadResponse>()?;

		if pages.is_empty() {
			bail!("No pages found for chapter {}", chapter.key);
		}

		Ok(pages
			.into_iter()
			.map(|url| Page {
				content: PageContent::url(url),
				..Default::default()
			})
			.collect())
	}
}

impl DeepLinkHandler for SenManga {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		const MANGA_PATH: &str = "/manga/";

		let Some(path) = url
			.strip_prefix(BASE_URL)
			.and_then(|path| path.strip_prefix(MANGA_PATH))
		else {
			return Ok(None);
		};

		let mut segments = path.trim_end_matches('/').split('/');
		let Some(manga_key) = segments.next().filter(|segment| !segment.is_empty()) else {
			return Ok(None);
		};

		// Series: /manga/<slug>/
		// Chapter: /manga/<slug>/chapter-<key>/
		Ok(Some(
			match segments.next().and_then(|it| it.strip_prefix("chapter-")) {
				Some(chapter_key) => DeepLinkResult::Chapter {
					manga_key: manga_key.into(),
					key: chapter_key.into(),
				},
				None => DeepLinkResult::Manga {
					key: manga_key.into(),
				},
			},
		))
	}
}

fn parse_status(status: Option<&str>) -> MangaStatus {
	match status {
		Some("Ongoing") => MangaStatus::Ongoing,
		Some("Completed") => MangaStatus::Completed,
		Some("Cancelled") => MangaStatus::Cancelled,
		Some("Hiatus") => MangaStatus::Hiatus,
		_ => MangaStatus::Unknown,
	}
}

fn content_rating(tags: &[String]) -> ContentRating {
	if tags.iter().any(|tag| NSFW_TAGS.contains(&tag.as_str())) {
		ContentRating::NSFW
	} else if tags
		.iter()
		.any(|tag| SUGGESTIVE_TAGS.contains(&tag.as_str()))
	{
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	}
}

register_source!(SenManga, DeepLinkHandler);
