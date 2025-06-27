#![no_std]
use aidoku::{
	AidokuError, Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Manga,
	MangaPageResult, Page, PageContent, Result, Source, Viewer,
	alloc::{String, Vec, vec},
	imports::{net::Request, std::send_partial_result},
	prelude::*,
};

const BASE_URL: &str = "https://tcbonepiecechapters.com";

struct TCBScans;

impl Source for TCBScans {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		_page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let html = Request::get(format!("{BASE_URL}/projects"))?.html()?;

		let projects = html
			.select(".bg-card.border.border-border.rounded.p-3.mb-3")
			.map(|els| {
				els.filter_map(|el| {
					let title_element = el.select_first("a.mb-3.text-white.text-lg.font-bold")?;
					Some(Manga {
						key: title_element.attr("href")?,
						title: title_element.text()?,
						cover: el
							.select_first(".w-24.h-24.object-cover.rounded-lg")
							.and_then(|img| img.attr("src")),
						..Default::default()
					})
				})
				.collect::<Vec<_>>()
			})
			.ok_or(AidokuError::message("Unable to find projects"))?;

		let entries = if let Some(query) = query {
			let query = query.to_lowercase();
			projects
				.into_iter()
				.filter(|manga| manga.title.to_lowercase().contains(&query))
				.collect()
		} else {
			projects
		};

		Ok(MangaPageResult {
			entries,
			has_next_page: false,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{BASE_URL}{}", manga.key);
		let html = Request::get(&url)?.html()?;

		if needs_details {
			let el = html
				.select_first(".order-1.bg-card.border.border-border.rounded.py-3")
				.ok_or(AidokuError::message("Unable to find manga details"))?;
			manga.title = el
				.select_first(".my-3.font-bold.text-3xl")
				.and_then(|e| e.text())
				.unwrap_or(manga.title);
			manga.cover = el
				.select_first(".flex.items-center.justify-center img")
				.and_then(|img| img.attr("src"));
			manga.description = el.select_first(".leading-6.my-3").and_then(|e| e.text());
			manga.url = Some(url);
			manga.content_rating = ContentRating::Safe;
			manga.viewer = Viewer::RightToLeft;
			send_partial_result(&manga);
		}

		if needs_chapters {
			manga.chapters = html
				.select(".bg-card.border.border-border.rounded.p-3.mb-3")
				.map(|els| {
					els.filter_map(|el| {
						let key = el.attr("href")?;
						let url = format!("{BASE_URL}{key}");
						Some(Chapter {
							key,
							title: el.select_first(".text-gray-500").and_then(|e| e.text()),
							chapter_number: el
								.select_first(".text-lg.font-bold:not(.flex)")
								.and_then(|e| e.text())
								.and_then(|s| {
									s.rsplit_once(' ').and_then(|(_, s)| s.parse::<f32>().ok())
								}),
							scanlators: Some(vec!["TCB Scans".into()]),
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
		let html = Request::get(format!("{BASE_URL}{}", chapter.key))?.html()?;

		Ok(html
			.select(".flex.flex-col.items-center.justify-center picture img")
			.map(|els| {
				els.filter_map(|el| {
					Some(Page {
						content: PageContent::url(el.attr("src")?),
						..Default::default()
					})
				})
				.collect::<Vec<_>>()
			})
			.unwrap_or_default())
	}
}

impl DeepLinkHandler for TCBScans {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};

		const MANGA_PATH: &str = "/mangas/";
		const CHAPTER_PATH: &str = "/chapters/";

		if path.starts_with(MANGA_PATH) {
			// ex: https://tcbonepiecechapters.com/mangas/5/one-piece
			Ok(Some(DeepLinkResult::Manga { key: path.into() }))
		} else if path.starts_with(CHAPTER_PATH) {
			// ex: https://tcbonepiecechapters.com/chapters/7868/one-piece-chapter-1153
			let html = Request::get(&url)?.html()?;
			let manga_key = html
				.select("main div.text-sm.font-bold > a")
    			.and_then(|mut els| els.next_back()) // "View all chapters" button
				.and_then(|a| a.attr("href"))
				.ok_or(AidokuError::message("Missing manga key"))?;

			Ok(Some(DeepLinkResult::Chapter {
				manga_key,
				key: path.into(),
			}))
		} else {
			Ok(None)
		}
	}
}

register_source!(TCBScans, DeepLinkHandler);
