use super::Params;
use crate::helper;
use aidoku::{
	alloc::{vec, String, Vec},
	helpers::date::parse_local_date,
	imports::{error::AidokuError, net::Request, std::send_partial_result},
	prelude::*,
	Chapter, ContentRating, DeepLinkResult, FilterItem, FilterValue, HomeComponent,
	HomeComponentValue, HomeLayout, Listing, Manga, MangaPageResult, MangaWithChapter, Page,
	PageContent, PageContext, Result, Viewer,
};

pub trait Impl {
	fn new() -> Self;

	fn params(&self) -> Params;

	fn get_manga_list(
		&self,
		params: &Params,
		listing: Listing,
		page: i32,
	) -> Result<MangaPageResult> {
		let (sort_index, extra_filter) = match listing.id.as_str() {
			"new" => (0, None),
			"latest" => (1, None),
			"hot" => (2, None),
			"completed" => (
				1,
				Some(FilterValue::Select {
					id: "status".into(),
					value: "Completed".into(),
				}),
			),
			_ => return Err(AidokuError::message("Invalid listing")),
		};

		let mut filters = Vec::with_capacity(if extra_filter.is_some() { 2 } else { 1 });
		filters.push(FilterValue::Sort {
			id: "sort".into(),
			index: sort_index,
			ascending: false,
		});
		if let Some(f) = extra_filter {
			filters.push(f);
		}

		self.get_search_manga_list(params, None, page, filters)
	}

	fn get_search_manga_list(
		&self,
		params: &Params,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let url = helper::get_search_url(params, query, page, filters);
		let html = Request::get(url)?
			.header("Referer", &format!("{}/", params.base_url))
			.html()?;

		let entries = html
			.select(params.item_selector.as_ref())
			.map(|els| {
				els.filter_map(|item| {
					let title = item
						.select_first(".story_name")
						.and_then(|el| el.text())
						.unwrap_or_else(|| {
							item.select_first("a")
								.and_then(|a| a.attr("title"))
								.unwrap_or_default()
						});
					let url = item.select_first("a")?.attr("href")?;
					let key = url
						.strip_prefix(params.base_url.as_ref())
						.unwrap_or(&url)
						.into();
					let cover = item.select_first("img").and_then(|img| img.attr("src"));
					Some(Manga {
						key,
						cover,
						title,
						url: Some(url),
						..Default::default()
					})
				})
				.collect::<Vec<_>>()
			})
			.unwrap_or_default();

		// last page link text in the format "Last(NUM)"
		let has_next_page = {
			let last_page = html
				.select_first("a.page_last")
				.and_then(|a| a.text())
				.and_then(|last_page_string| {
					last_page_string[5..last_page_string.len() - 1]
						.parse::<i32>()
						.ok()
				});
			last_page
				.map(|last| page < last)
				.unwrap_or_else(|| !entries.is_empty())
		};

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn get_manga_update(
		&self,
		params: &Params,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{}{}", params.base_url, manga.key);
		let html = Request::get(&url)?
			.header("Referer", &format!("{}/", params.base_url))
			.html()?;

		if needs_details {
			let details = html
				.select_first("div.manga-info-top, div.panel-story-info")
				.ok_or(AidokuError::message("Missing manga details"))?;
			manga.title = details
				.select_first("h1")
				.and_then(|h1| h1.text())
				.unwrap_or(manga.title);
			manga.cover = details
				.select_first("div.manga-info-pic img, span.info-image img")
				.and_then(|img| img.attr("src"));
			manga.authors = details
				.select("li:contains(author) a, td:containsOwn(author) + td a")
				.map(|els| els.filter_map(|el| el.text()).collect::<Vec<String>>());
			manga.description = html
				.select_first("div#contentBox")
				.and_then(|div| div.text())
				.map(|text| {
					text.replace(&format!("{} summary:", manga.title), "")
						.trim()
						.into()
				});
			manga.url = Some(url);
			manga.tags = details
				.select("div.manga-info-top li:contains(genres) a, td:containsOwn(genres) + td a")
				.map(|els| els.filter_map(|el| el.text()).collect::<Vec<String>>());
			manga.status = helper::status_from_string(
				&details
					.select_first("li:contains(status), td:containsOwn(status) + td")
					.and_then(|el| el.text())
					.unwrap_or_default(),
			);

			let tags = manga.tags.as_deref().unwrap_or(&[]);
			manga.content_rating = if tags
				.iter()
				.any(|e| matches!(e.as_str(), "Adult" | "Mature" | "Smut" | "Yaoi"))
			{
				ContentRating::NSFW
			} else if tags.iter().any(|e| e == "Ecchi") {
				ContentRating::Suggestive
			} else {
				ContentRating::Safe
			};

			manga.viewer = if tags
				.iter()
				.any(|e| matches!(e.as_str(), "Manhwa" | "Manhua" | "Webtoons" | "Long Strip"))
			{
				Viewer::Webtoon
			} else {
				Viewer::RightToLeft
			};

			send_partial_result(&manga);
		}

		if needs_chapters {
			manga.chapters = html
				.select("div.chapter-list div.row, ul.row-content-chapter li")
				.map(|els| {
					els.filter_map(|el| {
						let link = el.select_first("a")?;

						let url = link.attr("href")?;
						let key = url
							.strip_prefix(params.base_url.as_ref())
							.unwrap_or(&url)
							.into();
						let title = link.text().map(helper::strip_default_chapter_title);
						let chapter_number = helper::get_chapter_number(&url);
						let date_uploaded = el
							.select_first("span[title]")
							.and_then(|span| span.attr("title"))
							.and_then(|date| parse_local_date(date, "%b-%d-%Y %H:%M"));

						Some(Chapter {
							key,
							title,
							chapter_number,
							date_uploaded,
							url: Some(url),
							..Default::default()
						})
					})
					.collect()
				});
		}

		Ok(manga)
	}

	fn get_page_list(&self, params: &Params, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{}{}", params.base_url, chapter.key);

		let html = Request::get(url)?
			.header("Referer", &format!("{}/", params.base_url))
			.html()?;

		Ok(html
			.select("div.container-chapter-reader > img")
			.map(|els| {
				els.filter_map(|el| {
					Some(Page {
						content: PageContent::url(el.attr("src")?),
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default())
	}

	fn get_home(&self, params: &Params) -> Result<HomeLayout> {
		let html = Request::get(&params.base_url)?.html()?;

		Ok(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some("Popular Manga".into()),
					value: HomeComponentValue::Scroller {
						entries: html
							.select("#owl-demo .item")
							.map(|els| {
								els.filter_map(|el| {
									let link = el.select_first(".slide-caption > h3 > a")?;
									let href = link.attr("href")?;
									Some(
										Manga {
											key: href
												.strip_prefix(params.base_url.as_ref())?
												.into(),
											title: link.attr("title")?,
											cover: el.select_first("img")?.attr("src"),
											..Default::default()
										}
										.into(),
									)
								})
								.collect()
							})
							.unwrap_or_default(),
						listing: None,
					},
					..Default::default()
				},
				HomeComponent {
					title: Some("Latest Updates".into()),
					value: HomeComponentValue::MangaChapterList {
						page_size: None,
						entries: html
							.select("#contentstory .itemupdate")
							.map(|els| {
								els.filter_map(|el| {
									let manga_link = el.select_first("ul > li > h3 > a")?;
									let manga_href = manga_link.attr("href")?;
									let chpater_link = el.select_first("ul > li > span > a")?;
									Some(MangaWithChapter {
										manga: Manga {
											key: manga_href
												.strip_prefix(params.base_url.as_ref())?
												.into(),
											title: manga_link.text()?,
											cover: el.select_first("img")?.attr("src"),
											..Default::default()
										},
										chapter: Chapter {
											title: chpater_link.attr("title"),
											..Default::default()
										},
									})
								})
								.collect()
							})
							.unwrap_or_default(),
						listing: None,
					},
					..Default::default()
				},
				HomeComponent {
					title: Some("Genres".into()),
					value: HomeComponentValue::Filters(
						html.select("table > tbody > tr > td > a")
							.map(|els| {
								els.skip(6) // sort and status items
									.filter_map(|el| {
										let genre = el.attr("title")?;
										if !el.attr("href").is_some_and(|href| href.contains("genre/")) {
											// filter out tags on kakalot
											return None;
										}
										Some(FilterItem {
											title: genre.clone(),
											values: Some(vec![FilterValue::Select {
												id: "genre".into(),
												value: genre,
											}]),
										})
									})
									.collect()
							})
							.unwrap_or_default(),
					),
					..Default::default()
				},
			],
		})
	}

	fn get_image_request(
		&self,
		params: &Params,
		url: String,
		_context: Option<PageContext>,
	) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", &format!("{}/", params.base_url)))
	}

	fn handle_deep_link(&self, params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(params.base_url.as_ref()) else {
			return Ok(None);
		};

		const MANGA_PATH: &str = "manga/";
		if !path.starts_with(MANGA_PATH) {
			return Ok(None);
		}

		if let Some(idx) = path.rfind("/chapter-") {
			// ex: https://www.manganato.gg/manga/im-a-villain-but-im-favored/chapter-1
			let manga_key = &path[..idx];
			Ok(Some(DeepLinkResult::Chapter {
				manga_key: manga_key.into(),
				key: path.into(),
			}))
		} else {
			// ex: https://www.manganato.gg/manga/im-a-villain-but-im-favored
			Ok(Some(DeepLinkResult::Manga { key: path.into() }))
		}
	}
}
