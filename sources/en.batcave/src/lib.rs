#![no_std]
use aidoku::{
	alloc::{string::ToString, vec, String, Vec},
	imports::{
		net::Request,
		std::{parse_date, send_partial_result},
	},
	prelude::*,
	AidokuError, Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider,
	Manga, MangaPageResult, MangaStatus, Page, PageContent, Result, Source,
};
use regex::Regex;
use serde::Deserialize;

mod home;

const BASE_URL: &str = "https://batcave.biz";
const REFERER: &str = "https://batcave.biz/";

struct BatCave;

#[derive(Deserialize)]
struct ChapterList {
	news_id: i32,
	chapters: Vec<SingleChapter>,
}
#[derive(Deserialize)]
struct SingleChapter {
	date: String,
	id: i32,
	title: String,
}
#[derive(Deserialize)]
struct PageList {
	images: Vec<String>,
}

impl Source for BatCave {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut filters_vec = Vec::<String>::new();

		for filter in filters {
			match filter {
				FilterValue::Range { id, from, to } => {
					if id.as_str() == "year_of_issue" {
						if let Some(from) = from {
							filters_vec.push(format!("y[from]={}", from));
						}
						if let Some(to) = to {
							filters_vec.push(format!("y[to]={}", to));
						}
					}
				}
				FilterValue::MultiSelect { id, included, .. } => {
					if id.as_str() == "genre" {
						filters_vec.push(format!("g={}", included.join(",")));
					}
				}
				_ => {}
			}
		}

		let url = if filters_vec.is_empty() {
			format!(
				"{BASE_URL}/search/{}/page/{page}/",
				query.unwrap_or_default()
			)
		} else {
			format!(
				"{BASE_URL}/ComicList/{}/page/{page}/",
				filters_vec.join("/")
			)
		};

		let result = Request::get(&url)?.html()?;

		let entries = result
			.select("#dle-content > div:not(.pagination)")
			.map(|elements| {
				elements
					.filter_map(|element| {
						let cover = element.select_first("img")?.attr("abs:data-src");
						let url = element.select_first("a")?.attr("abs:href");
						let title = element
							.select_first("div > h2")
							.and_then(|x| x.text())
							.unwrap_or_default();

						Some(Manga {
							key: url.clone().unwrap_or_default(),
							cover,
							title,
							url,
							..Default::default()
						})
					})
					.collect::<Vec<Manga>>()
			})
			.unwrap_or_default();

		let has_next_page = !entries.is_empty();

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
		let html = Request::get(&manga.key)?.html()?;

		if needs_details {
			manga.title = html
				.select_first("header h1")
				.and_then(|x| x.text())
				.unwrap_or_default();

			manga.description = html.select_first(".page__text").and_then(|x| x.text());

			manga.cover = html
				.select_first(".page__poster img")
				.and_then(|x| x.attr("abs:src"));

			manga.artists = html
				.select_first("ul > li:has(div:contains(Artist))")
				.and_then(|x| x.text())
				.and_then(|x| x.strip_prefix("Artist: ").map(|x| x.to_string()))
				.map(|x| vec![x]);

			manga.authors = html
				.select_first("ul > li:has(div:contains(Writer))")
				.and_then(|x| x.text())
				.and_then(|x| x.strip_prefix("Writer: ").map(|x| x.to_string()))
				.map(|x| vec![x]);

			manga.tags = html.select(".page__tags > a").map(|elements| {
				elements
					.map(|element| element.text().unwrap_or_default())
					.collect::<Vec<String>>()
			});

			let status_str = html
				.select_first("ul > li:has(div:contains(Release type))")
				.and_then(|x| x.text())
				.unwrap_or_default();

			manga.status = match status_str
				.strip_prefix("Release type: ")
				.unwrap_or_default()
			{
				"Completed" | "Complete" => MangaStatus::Completed,
				"Ongoing" => MangaStatus::Ongoing,
				_ => MangaStatus::Unknown,
			};

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let script_data = html
				.select_first(".page__chapters-list > script")
				.and_then(|x| x.data())
				.ok_or(error!("No script data"))?;

			let json_str = script_data
				.strip_prefix("window.__DATA__ = ")
				.and_then(|x| x.strip_suffix(";"))
				.unwrap_or_default();

			let chapter_list = serde_json::from_str::<ChapterList>(json_str)
				.map_err(AidokuError::JsonParseError)?;

			let chapters = chapter_list
				.chapters
				.into_iter()
				.map(|chapter| {
					let url = format!("{BASE_URL}/reader/{}/{}", chapter_list.news_id, chapter.id);

					let title = chapter
						.title
						.strip_prefix(&manga.title)
						.map(str::trim)
						.map(String::from)
						.unwrap_or_else(|| chapter.title);

					let chapter_number = title
						.find('#')
						.and_then(|idx| title[idx + 1..].parse::<f32>().ok());

					let date_uploaded = parse_date(&chapter.date, "dd.MM.yyyy");

					Chapter {
						key: url.clone(),
						url: Some(url),
						title: Some(title),
						chapter_number,
						date_uploaded,
						..Default::default()
					}
				})
				.collect::<Vec<Chapter>>();

			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let html = Request::get(&chapter.key)?.html()?;

		let pages = html
			.select("script")
			.map(|elements| {
				elements
					.filter_map(|element| {
						let text = element.data()?;
						if !text.starts_with("window.__DATA__") {
							return None;
						}

						let page_json_str = text
							.strip_prefix("window.__DATA__ = ")
							.and_then(|x| x.strip_suffix(";"))
							.unwrap_or_default();

						let page_list = serde_json::from_str::<PageList>(page_json_str).ok()?;

						let pages = page_list
							.images
							.into_iter()
							.map(|page_url| Page {
								content: PageContent::url(page_url),
								..Default::default()
							})
							.collect::<Vec<Page>>();

						Some(pages)
					})
					.flatten()
					.collect::<Vec<Page>>()
			})
			.unwrap_or_default();

		Ok(pages)
	}
}

impl ImageRequestProvider for BatCave {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", REFERER))
	}
}

impl DeepLinkHandler for BatCave {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let re = match Regex::new(r"^https://batcave\.biz/\d+-[\w-]+\.html$") {
			Ok(re) => re,
			Err(_) => return Ok(None),
		};

		if re.is_match(&url) {
			return Ok(Some(DeepLinkResult::Manga { key: url }));
		}

		Ok(None)
	}
}

register_source!(BatCave, ImageRequestProvider, Home, DeepLinkHandler);
