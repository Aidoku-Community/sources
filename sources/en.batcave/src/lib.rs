#![no_std]
use aidoku::{
	alloc::{string::ToString, vec, Box, String, Vec},
	imports::html::Document,
	imports::net::Request,
	imports::std::{parse_date, send_partial_result},
	prelude::*,
	Chapter, FilterValue, Home, HomeComponent, HomeLayout, ImageRequestProvider, Link, Manga,
	MangaPageResult, MangaStatus, MangaWithChapter, Page, PageContent, Result, Source,
};
use serde::Deserialize;

const BASE_URL: &str = "https://batcave.biz";
const REFERER: &str = "https://batcave.biz/";

type ComponentBuilderFn = Box<dyn Fn(&Document) -> Option<HomeComponent>>;

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
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let url = format!(
			"{BASE_URL}/search/{}/page/{page}/",
			query.unwrap_or_default()
		);
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
				.map(|x| x.strip_prefix("Artist: ").unwrap_or_default().to_string())
				.map(|x| vec![x]);

			manga.authors = html
				.select_first("ul > li:has(div:contains(Writer))")
				.and_then(|x| x.text())
				.map(|x| x.strip_prefix("Writer: ").unwrap_or_default().to_string())
				.map(|x| vec![x]);

			let status_str = html
				.select_first("ul > li:has(div:contains(Release type))")
				.and_then(|x| x.text())
				.unwrap_or_default();

			manga.status = match status_str
				.strip_prefix("Release type: ")
				.unwrap_or_default()
			{
				"Completed" => MangaStatus::Completed,
				"Complete" => MangaStatus::Completed,
				"Ongoing" => MangaStatus::Ongoing,
				_ => MangaStatus::Unknown,
			};

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let chapter_list: ChapterList = serde_json::from_str(
				html.select_first(".page__chapters-list > script")
					.and_then(|x| x.data())
					.expect("No script data")
					.strip_prefix("window.__DATA__ = ")
					.expect("Wrong script format")
					.strip_suffix(";")
					.unwrap_or_default(),
			)
			.unwrap();

			manga.chapters = Some(
				chapter_list
					.chapters
					.into_iter()
					.map(|chapter| {
						let url =
							format!("{BASE_URL}/reader/{}/{}", chapter_list.news_id, chapter.id);

						let title = chapter
							.title
							.strip_prefix(&manga.title)
							.map(str::trim)
							.map(String::from)
							.unwrap_or_else(|| chapter.title);

						let mut chapter_number = None;
						if let Some(idx) = title.find('#') {
							chapter_number = title[idx + 1..].parse::<f32>().ok();
						}

						let date_uploaded = parse_date(&chapter.date, "%-d.%-m.%Y");

						Chapter {
							key: url.clone(),
							url: Some(url),
							title: Some(title),
							chapter_number,
							date_uploaded,
							..Default::default()
						}
					})
					.collect::<Vec<Chapter>>(),
			);
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

						let page_list: PageList = serde_json::from_str(
							text.strip_prefix("window.__DATA__ = ")
								.and_then(|x| x.strip_suffix(";"))
								.unwrap_or_default(),
						)
						.unwrap();

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

fn get_home_hot_releases(html: &Document) -> Option<HomeComponent> {
	let title = html
		.select_first(".sect--hot > .sect__title")
		.and_then(|x| x.text());

	let entries = html
		.select("section.sect--hot > .sect__content > a.grid-item")
		.map(|elements| {
			elements
				.map(|element| {
					let title = element
						.select_first("div > p")
						.and_then(|x| x.text())
						.unwrap_or_default();

					let cover = element
						.select_first("img")
						.and_then(|x| x.attr("abs:data-src"));

					let url = element.attr("abs:href");

					Manga {
						key: url.clone().unwrap_or_default(),
						cover,
						title,
						url,
						..Default::default()
					}
				})
				.map(Into::into)
				.collect::<Vec<Link>>()
		})
		.unwrap_or_default();

	if !entries.is_empty() {
		Some(HomeComponent {
			title,
			value: aidoku::HomeComponentValue::Scroller {
				entries,
				listing: None,
			},
			..Default::default()
		})
	} else {
		None
	}
}

fn get_home_newest_releases(html: &Document) -> Option<HomeComponent> {
	let title = html
		.select_first(".sect--latest > .sect__title")
		.and_then(|x| x.text());

	let entries = html
		.select(".sect--latest > .sect__content > li.latest")
		.map(|elements| {
			elements
				.map(|element| {
					let manga_url = element
						.select_first(".latest__title")
						.and_then(|x| x.attr("abs:href"));

					let chapter_url = element
						.select_first(".latest__chapter > a")
						.and_then(|x| x.attr("abs:href"));

					let cover = element
						.select_first(".latest__img > img")
						.and_then(|x| x.attr("abs:src"));

					let manga_title = element
						.select_first(".latest__title")
						.and_then(|x| x.text())
						.unwrap_or_default();

					let details_text = element
						.select_first(".latest__chapter")
						.and_then(|x| x.text());

					let mut date_uploaded = None;
					let mut chapter_title = None;
					let mut chapter_number = None;

					if let Some(text) = details_text {
						let parts = text.splitn(2, " - ").collect::<Vec<&str>>();
						if parts.len() == 2 {
							date_uploaded = parse_date(parts[0].trim(), "%-d.%-m.%Y");

							chapter_title = parts[1]
								.strip_prefix(&manga_title)
								.map(str::trim)
								.map(String::from);

							if let Some(idx) = parts[1].find('#') {
								chapter_number = parts[1][idx + 1..].parse::<f32>().ok();
							}
						}
					}

					MangaWithChapter {
						manga: Manga {
							key: manga_url.clone().unwrap_or_default(),
							cover,
							title: manga_title,
							url: manga_url,
							..Default::default()
						},
						chapter: Chapter {
							key: chapter_url.clone().unwrap_or_default(),
							url: chapter_url,
							title: chapter_title,
							chapter_number,
							date_uploaded,
							..Default::default()
						},
					}
				})
				.collect::<Vec<MangaWithChapter>>()
		})
		.unwrap_or_default();

	Some(HomeComponent {
		title,
		value: aidoku::HomeComponentValue::MangaChapterList {
			page_size: Some(6),
			entries,
			listing: None,
		},
		..Default::default()
	})
}

fn get_side_block(index: i32) -> ComponentBuilderFn {
	Box::new(move |html: &Document| {
		let block = html.select_first(format!(".side-block:nth-of-type({})", index))?;
		let title = block.select_first(".side-block__title")?.text();

		let entries = block
			.select(".side-block__content > a")
			.map(|elements| {
				elements
					.map(|element| {
						let title = element
							.select_first(".popular__title")
							.and_then(|x| x.text())
							.unwrap_or_default();

						let cover = element
							.select_first("img")
							.and_then(|x| x.attr("abs:data-src"));

						let url = element.attr("abs:href");

						Manga {
							key: url.clone().unwrap_or_default(),
							cover,
							title,
							url,
							..Default::default()
						}
					})
					.map(Into::into)
					.collect::<Vec<Link>>()
			})
			.unwrap_or_default();

		Some(HomeComponent {
			title,
			value: aidoku::HomeComponentValue::MangaList {
				ranking: false,
				entries,
				listing: None,
				page_size: None,
			},
			..Default::default()
		})
	})
}

impl Home for BatCave {
	fn get_home(&self) -> Result<HomeLayout> {
		let html = Request::get(BASE_URL)?.html()?;
		let mut components = Vec::new();

		let component_fns: Vec<ComponentBuilderFn> = vec![
			Box::new(get_home_hot_releases),
			Box::new(get_home_newest_releases),
			get_side_block(1),
			get_side_block(2),
		];

		for component_fn in component_fns {
			if let Some(component) = component_fn(&html) {
				components.push(component)
			}
		}

		Ok(HomeLayout { components })
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

register_source!(BatCave, ImageRequestProvider, Home);
