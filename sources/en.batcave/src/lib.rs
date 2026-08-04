#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, HashMap, ImageRequestProvider, Manga,
	MangaPageResult, MangaStatus, Page, PageContent, Result, Source, WebLoginHandler,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::encode_uri_component,
	imports::net::Request,
	prelude::*,
};

mod helpers;
mod home;
mod models;

use models::*;

use crate::helpers::BatCaveHtml;

const BASE_URL: &str = "https://batcave.biz";
const REFERER: &str = "https://batcave.biz/";
const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) \
                          AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 \
                          Mobile/15E148 Safari/604.1";
const VERIFY_KEY: &str = "verify";
const TRUST_COOKIE_KEY: &str = "__guard_trust";

struct BatCave;

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
		let url = if let Some(query) = query {
			format!(
				"{BASE_URL}/search/{}/page/{page}/",
				encode_uri_component(query)
			)
		} else {
			let mut filters_vec = Vec::<String>::new();
			for filter in filters {
				match filter {
					FilterValue::Range { from, to, .. } => {
						if let Some(from) = from {
							filters_vec.push(format!("y[from]={}", from));
						}
						if let Some(to) = to {
							filters_vec.push(format!("y[to]={}", to));
						}
					}
					FilterValue::MultiSelect { included, .. } => {
						filters_vec.push(format!("g={}", included.join(",")));
					}
					_ => {}
				}
			}
			if !filters_vec.is_empty() {
				format!(
					"{BASE_URL}/ComicList/{}/page/{page}/",
					filters_vec.join("/")
				)
			} else {
				format!(
					"{BASE_URL}/comix/{}",
					if page > 1 {
						format!("page/{page}/")
					} else {
						String::new()
					}
				)
			}
		};

		let html = Request::get(&url)?.batcave_html()?;

		let entries = html
			.select("#dle-content > .readed")
			.map(|elements| {
				elements
					.filter_map(|element| {
						let link = element.select_first(".readed__title > a")?;
						let url = link.attr("abs:href")?;
						let key = url.strip_prefix(BASE_URL)?.to_string();
						let cover = element.select_first("img")?.attr("abs:data-src");
						let title = link.own_text()?;
						Some(Manga {
							key,
							cover,
							title,
							url: Some(url),
							..Default::default()
						})
					})
					.collect::<Vec<Manga>>()
			})
			.unwrap_or_default();

		let has_next_page = html
			.select_first("div.pagination__pages")
			.and_then(|el| el.children().next_back())
			.map(|child| child.tag_name().as_deref() == Some("a"))
			.unwrap_or_default();

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
		let url = format!("{BASE_URL}{}", manga.key);
		let html = Request::get(&url)?.batcave_html()?;

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

			let chapter_list = serde_json::from_str::<ChapterList>(json_str)?;

			let chapters = chapter_list
				.chapters
				.into_iter()
				.map(|chapter| chapter.into_chapter(chapter_list.news_id, &manga.title))
				.collect::<Vec<Chapter>>();

			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{BASE_URL}{}", chapter.key);
		let html = Request::get(&url)?.batcave_html()?;

		let pages = html
			.select("script")
			.map(|elements| {
				elements
					.filter_map(|element| {
						let text = element.data()?;
						if !text.starts_with("window.__DATA__") {
							return None;
						}

						let page_json_str =
							text.strip_prefix("window.__DATA__ = ")?.strip_suffix(";")?;

						let page_list = serde_json::from_str::<PageList>(page_json_str).ok()?;

						let pages = page_list
							.images
							.into_iter()
							.map(|page_url| {
								let url = if page_url.starts_with("/") {
									format!("{BASE_URL}{}", page_url)
								} else {
									page_url
								};
								Page {
									content: PageContent::url(url),
									..Default::default()
								}
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

impl WebLoginHandler for BatCave {
	fn handle_web_login(&self, key: String, cookies: HashMap<String, String>) -> Result<bool> {
		Ok(key == VERIFY_KEY && cookies.contains_key(TRUST_COOKIE_KEY))
	}
}

impl ImageRequestProvider for BatCave {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		if url.contains("batcave.biz") {
			Ok(Request::get(url)?.header("Referer", REFERER))
		} else {
			Ok(Request::get(url)?)
		}
	}
}

impl DeepLinkHandler for BatCave {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(key) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};
		let Some((id, slug)) = key.strip_prefix('/').and_then(|path| path.split_once('-')) else {
			return Ok(None);
		};
		let Some(slug) = slug.strip_suffix(".html") else {
			return Ok(None);
		};

		if id.is_empty()
			|| slug.is_empty()
			|| !id.bytes().all(|byte| byte.is_ascii_digit())
			|| !slug
				.bytes()
				.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
		{
			return Ok(None);
		}

		Ok(Some(DeepLinkResult::Manga {
			key: key.to_string(),
		}))
	}
}

register_source!(
	BatCave,
	Home,
	ImageRequestProvider,
	DeepLinkHandler,
	WebLoginHandler
);
