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

		if let Some(query) = query.as_deref().filter(|query| !query.is_empty()) {
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
			manga.status = parse_status(status.as_deref());
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

#[cfg(test)]
mod test {
	use super::*;
	use aidoku::alloc::vec;
	use aidoku_test::aidoku_test;

	/// A long-running series, so its chapter list stays large enough to assert on.
	const TEST_MANGA_KEY: &str = "one-piece";

	#[aidoku_test]
	fn browse_test() {
		let source = SenManga;
		let result = source
			.get_search_manga_list(None, 1, vec![])
			.expect("get_search_manga_list failed");

		assert!(result.entries.len() >= 10);
		// The directory spans hundreds of pages, so the first one is never last.
		assert!(result.has_next_page);

		for manga in result.entries {
			assert!(!manga.key.is_empty());
			assert!(!manga.title.is_empty());
			assert!(
				manga
					.cover
					.as_deref()
					.is_some_and(|cover| cover.starts_with("https://"))
			);
		}
	}

	#[aidoku_test]
	fn search_test() {
		let source = SenManga;

		// The api matches against alternative titles too, so a japanese query
		// has to reach the romanized entry.
		let result = source
			.get_search_manga_list(Some("ワンピース".into()), 1, vec![])
			.expect("get_search_manga_list failed");

		assert!(!result.entries.is_empty());
		assert!(
			result
				.entries
				.iter()
				.any(|manga| manga.key == TEST_MANGA_KEY)
		);
	}

	#[aidoku_test]
	fn manga_details_test() {
		let source = SenManga;
		let manga = source
			.get_manga_update(
				Manga {
					key: TEST_MANGA_KEY.into(),
					..Default::default()
				},
				true,
				true,
			)
			.expect("get_manga_update failed");

		assert!(!manga.title.is_empty());
		assert!(manga.cover.is_some());
		assert!(manga.description.is_some());
		assert!(manga.tags.as_ref().is_some_and(|tags| !tags.is_empty()));
		assert_eq!(manga.viewer, Viewer::RightToLeft);

		let chapters = manga.chapters.expect("no chapters");
		assert!(chapters.len() >= 100);

		for chapter in chapters {
			assert!(!chapter.key.is_empty());
			assert!(chapter.chapter_number.is_some());
			// Setting a language would hide every chapter behind the app's
			// language filter, since this source is japanese-only.
			assert!(chapter.language.is_none());
		}
	}

	#[aidoku_test]
	fn page_list_test() {
		let source = SenManga;
		let manga = source
			.get_manga_update(
				Manga {
					key: TEST_MANGA_KEY.into(),
					..Default::default()
				},
				false,
				true,
			)
			.expect("get_manga_update failed");
		let chapter = manga
			.chapters
			.as_ref()
			.and_then(|chapters| chapters.first())
			.expect("no chapters")
			.clone();

		let pages = source
			.get_page_list(manga, chapter)
			.expect("get_page_list failed");

		assert!(!pages.is_empty());
		for page in pages {
			match page.content {
				PageContent::Url(url, _) => assert!(url.starts_with("https://")),
				_ => panic!("expected a url page"),
			}
		}
	}

	#[aidoku_test]
	fn deep_link_test() {
		let source = SenManga;

		let result = source
			.handle_deep_link(format!("{BASE_URL}/manga/{TEST_MANGA_KEY}/"))
			.expect("handle_deep_link failed");
		assert_eq!(
			result,
			Some(DeepLinkResult::Manga {
				key: TEST_MANGA_KEY.into()
			})
		);

		let result = source
			.handle_deep_link(format!(
				"{BASE_URL}/manga/{TEST_MANGA_KEY}/chapter-8.338323/"
			))
			.expect("handle_deep_link failed");
		assert_eq!(
			result,
			Some(DeepLinkResult::Chapter {
				manga_key: TEST_MANGA_KEY.into(),
				key: "8.338323".into()
			})
		);

		let result = source
			.handle_deep_link(format!("{BASE_URL}/directory"))
			.expect("handle_deep_link failed");
		assert_eq!(result, None);
	}
}
