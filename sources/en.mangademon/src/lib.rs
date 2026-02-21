#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Manga, MangaPageResult, MangaStatus,
	Page, PageContent, Result, Source, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::{QueryParameters, encode_uri_component},
	imports::{net::Request, std::parse_date_with_options},
	prelude::*,
};

mod helpers;
use helpers::*;

const BASE_URL: &str = "https://demonicscans.org";

struct MangaDemon;

impl Source for MangaDemon {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> aidoku::Result<aidoku::MangaPageResult> {
		let url = if let Some(query) = &query {
			format!(
				"{BASE_URL}/search.php?manga={}",
				encode_uri_component(query)
			)
		} else {
			let mut qs = QueryParameters::new();
			qs.push("list", Some(&page.to_string()));
			qs.push("orderby", Some("VIEWS DESC")); // sort must be present for genres to work
			for filter in filters {
				match filter {
					FilterValue::Sort {
						id,
						index,
						ascending,
					} => {
						let sort = match index {
							0 => "VIEWS",
							1 => "ID",
							2 => "NAME",
							_ => "VIEWS",
						};
						let asc = if ascending { "ASC" } else { "DESC" };
						qs.set(&id, Some(&format!("{sort} {asc}")));
					}
					FilterValue::Select { id, value } => {
						qs.push(&id, Some(&value));
					}
					FilterValue::MultiSelect { id, included, .. } => {
						for value in included {
							qs.push(&id, Some(&value));
						}
					}
					_ => {}
				}
			}
			format!("{BASE_URL}/advanced.php?{qs}")
		};

		let html = Request::get(url)?.html()?;

		let entries = html
			.select(if query.is_some() {
				"body > a[href]"
			} else {
				"div#advanced-content > div.advanced-element"
			})
			.map(|els| {
				els.filter_map(|el| {
					let link = el.select_first("a")?;
					let url = link.attr("abs:href")?;
					Some(Manga {
						key: get_manga_id(&url)?,
						title: if query.is_some() {
							el.select_first("div.seach-right > div")?.own_text()?
						} else {
							link.attr("title")?
						},
						cover: el.select_first("img")?.attr("abs:src"),
						url: Some(url),
						..Default::default()
					})
				})
				.collect()
			})
			.ok_or_else(|| error!("Failed to select elements"))?;

		let has_next_page = if query.is_none() {
			html.select_first("div.pagination > ul > a > li:contains(Next)")
				.is_some()
		} else {
			false
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
		let url = get_manga_url(&manga.key);
		let html = Request::get(&url)?.html()?;

		if needs_details {
			let container = html
				.select_first("div#manga-info-container")
				.ok_or_else(|| error!("Missing info container"))?;

			manga.title = container
				.select_first("h1.big-fat-titles")
				.and_then(|el| el.text())
				.unwrap_or(manga.title);
			manga.cover = container
				.select_first("div#manga-page img")
				.and_then(|el| el.attr("abs:src"));
			manga.authors = container
				.select_first("div#manga-info-stats > div:contains(Author) > li:nth-child(2)")
				.and_then(|el| el.text())
				.map(|s| vec![s]);
			manga.description = container
				.select_first("div#manga-info-rightColumn > div > div.white-font")
				.and_then(|el| el.text());
			manga.url = Some(url);
			manga.tags = container
				.select("div.genres-list > li")
				.map(|els| els.filter_map(|el| el.text()).collect());
			manga.status = container
				.select_first("div#manga-info-stats > div:contains(Status) > li:nth-child(2)")
				.and_then(|el| el.text())
				.map(|s| match s.to_ascii_lowercase().trim() {
					"ongoing" => MangaStatus::Ongoing,
					"completed" => MangaStatus::Completed,
					_ => MangaStatus::Unknown,
				})
				.unwrap_or_default();
			manga.viewer = Viewer::Webtoon;
		}

		if needs_chapters {
			manga.chapters = html.select("div#chapters-list a.chplinks").map(|els| {
				els.filter_map(|el| {
					let url = el.attr("abs:href")?;
					Some(Chapter {
						key: get_chapter_id(&url)?,
						chapter_number: el
							.own_text()?
							.strip_prefix("Chapter ")
							.and_then(|s| s.parse().ok()),
						date_uploaded: el
							.select_first("span")
							.and_then(|el| el.own_text())
							.and_then(|s| {
								parse_date_with_options(s, "yyyy-MM-dd", "en_US", "current")
							}),
						url: Some(url),
						..Default::default()
					})
				})
				.collect()
			});
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let html = Request::get(get_chapter_url(&chapter.key))?.html()?;

		html.select("img.imgholder")
			.map(|els| {
				els.filter_map(|el| {
					let url = el.attr("abs:src")?;
					Some(Page {
						content: PageContent::url(url),
						..Default::default()
					})
				})
				.collect()
			})
			.ok_or_else(|| error!("Failed to select page elements"))
	}
}

impl DeepLinkHandler for MangaDemon {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		Ok(get_manga_id(&url).map(|key| DeepLinkResult::Manga { key }))
	}
}

register_source!(MangaDemon, DeepLinkHandler);
