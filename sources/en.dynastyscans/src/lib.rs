#![no_std]
use aidoku::{
	AidokuError, Chapter, CoverImageProcessor, DeepLinkHandler, DeepLinkResult, FilterValue,
	ImageResponse, Manga, MangaPageResult, MigrationHandler, Page, Result, Source,
	alloc::{String, Vec, rc::Rc, string::ToString},
	helpers::uri::QueryParameters,
	imports::{canvas::ImageRef, defaults::defaults_get, net::Request},
	prelude::*,
};

const BASE_URL: &str = "https://dynasty-scans.com";
const SERIES_PATH: &str = "/series/";
const CHAPTERS_PATH: &str = "/chapters/";

mod models;
use models::*;

struct DynastyScans;

impl Source for DynastyScans {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut qs = QueryParameters::new();

		if query.is_some() {
			qs.push("q", query.as_deref());
		}

		if page > 1 {
			qs.push("page", Some(&page.to_string()));
		}

		let mut has_group_filter = false;

		for filter in filters {
			match filter {
				FilterValue::Sort { index, .. } => {
					let option = match index {
						0 => "",
						1 => "name",
						2 => "created_at",
						_ => "",
					};
					qs.push_encoded("sort", Some(option));
				}
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} => match id.as_str() {
					"tags" => {
						for id in included {
							qs.push("with[]", Some(&id));
						}
						for id in excluded {
							qs.push("without[]", Some(&id));
						}
					}
					"type" => {
						for id in included {
							qs.push("classes[]", Some(&id));
						}
						has_group_filter = true;
					}
					_ => {}
				},
				_ => continue,
			}
		}

		if !has_group_filter {
			qs.push("classes[]", Some("Series"));
		}
		let url = format!("{BASE_URL}/search?{qs}");

		let skip_images = defaults_get::<bool>("skipImages").unwrap_or_default();

		let html = Request::get(url)?.html()?;

		let entries = html
			.select(".chapter-list a.name")
			.map(|els| {
				els.flat_map(|el| {
					let key = el.attr("href")?.strip_prefix(SERIES_PATH)?.into();
					let cover =
						(!skip_images).then_some(format!("{BASE_URL}{SERIES_PATH}{key}.json"));
					Some(Manga {
						key,
						title: el.text()?,
						cover,
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default();

		let has_next_page = html
			.select_first("div.pagination > ul > li > a[rel=next]")
			.is_some();

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
		let url = format!("{BASE_URL}{SERIES_PATH}{}.json", manga.key);
		let mut entry = Request::get(url)?.json_owned::<MangaEntry>()?;

		if needs_chapters {
			manga.chapters = entry.chapters();
		}

		if needs_details {
			manga.copy_from(entry.into());
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		Ok(
			Request::get(format!("{BASE_URL}{CHAPTERS_PATH}{}.json", chapter.key))?
				.json_owned::<ChapterResponse>()?
				.pages(),
		)
	}
}

impl CoverImageProcessor for DynastyScans {
	fn process_cover_image(&self, response: ImageResponse) -> Result<ImageRef> {
		let Some(url) = response.request.url else {
			bail!("Missing cover request URL")
		};
		if url.ends_with("json") {
			let data = response.image.data();
			let entry = serde_json::from_slice::<serde_json::Value>(&data)
				.map_err(|err| AidokuError::JsonParseError(Rc::new(err)))?;
			let Some(cover) = entry["cover"].as_str() else {
				bail!("No cover image for {url}");
			};
			Ok(Request::get(format!("{BASE_URL}{cover}"))?.image()?)
		} else {
			Ok(response.image)
		}
	}
}

impl MigrationHandler for DynastyScans {
	fn handle_manga_migration(&self, key: String) -> Result<String> {
		Ok(key.trim_start_matches("series/").into())
	}

	fn handle_chapter_migration(&self, _manga_key: String, chapter_key: String) -> Result<String> {
		Ok(chapter_key)
	}
}

impl DeepLinkHandler for DynastyScans {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};

		let path = path
			.split(['?', '#'])
			.next()
			.unwrap_or_default()
			.trim_end_matches('/');

		fn valid_key(key: &str) -> bool {
			!key.is_empty()
				&& key
					.bytes()
					.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
		}

		// https://dynasty-scans.com/series/gabriel_dropout
		if let Some(key) = path.strip_prefix(SERIES_PATH).filter(|key| valid_key(key)) {
			return Ok(Some(DeepLinkResult::Manga { key: key.into() }));
		}

		// https://dynasty-scans.com/chapters/gabriel_dropout_ch138
		let Some(chapter_key) = path
			.strip_prefix(CHAPTERS_PATH)
			.filter(|key| valid_key(key))
		else {
			return Ok(None);
		};
		let Some((manga_key, chapter_number)) = chapter_key.rsplit_once("_ch") else {
			return Ok(None);
		};
		if manga_key.is_empty()
			|| chapter_number.is_empty()
			|| !chapter_number.bytes().all(|byte| byte.is_ascii_digit())
		{
			return Ok(None);
		}

		Ok(Some(DeepLinkResult::Chapter {
			manga_key: manga_key.into(),
			key: chapter_key.into(),
		}))
	}
}

register_source!(
	DynastyScans,
	CoverImageProcessor,
	MigrationHandler,
	DeepLinkHandler
);
