#![no_std]
use aidoku::{
	Chapter, FilterValue, HomeComponent, HomeComponentValue, HomeLayout, Listing, Manga,
	MangaPageResult, MangaWithChapter, Page, PageContent, Result, Source,
	alloc::{Vec, format, string::String, vec},
	helpers::uri::encode_uri_component,
	imports::{
		html::{Document, Element},
		net::Request,
		std::parse_date,
	},
	prelude::*,
};
use mmrcms::{
	Impl, MMRCMS, Params,
	helpers::{self, ElementImageAttr},
};

const BASE_URL: &str = "https://readcomicsonline.ru";

struct ReadComicsOnline;

impl Impl for ReadComicsOnline {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			item_path: "comic".into(),
			supports_advanced_search: true,
			details_title_selector: "h1.text-2xl".into(),
			manga_list_selector: "main > .grid > .group".into(),
			manga_list_next_page_selector: "nav a[rel=next]".into(),
			chapter_list_selector: ".overflow-hidden.border-ink-600 > a".into(),
			chapter_string: "".into(),
			date_format: "d MMM yyyy".into(),
			..Default::default()
		}
	}

	fn parse_manga_element(&self, params: &Params, element: &Element) -> Option<Manga> {
		let anchor = element.select_first("a")?;
		let key: String = anchor
			.attr("abs:href")?
			.strip_prefix(&format!("{}/{}/", params.base_url, params.item_path))?
			.into();
		let cover = helpers::guess_cover(
			&params.base_url,
			&params.item_path,
			&key,
			element.select_first("img").and_then(|img| img.attr("src")),
		);
		Some(Manga {
			key,
			title: anchor.select_first("p")?.text()?,
			cover: Some(cover),
			..Default::default()
		})
	}

	fn parse_manga_details(&self, params: &Params, html: &Document, manga: &mut Manga, url: &str) {
		manga.title = html
			.select_first("h1.text-2xl")
			.and_then(|el| el.text())
			.unwrap_or_default();
		manga.cover = html
			.select_first("img.w-full.rounded-xl")
			.and_then(|img| img.img_attr())
			.map(|cover| {
				helpers::guess_cover(&params.base_url, &params.item_path, &manga.key, Some(cover))
			});
		manga.description = html.select_first("p.mt-5.text-sm").and_then(|el| el.text());
		manga.status = html
			.select_first("div.flex.flex-wrap.gap-2 span.rounded-full")
			.and_then(|el| el.text())
			.map(|value| helpers::status(&value))
			.unwrap_or_default();
		manga.tags = html
			.select("dl div:contains(Genres:) a")
			.map(|els| els.filter_map(|el| el.text()).collect());
		manga.authors = html
			.select("div:has(span:contains(Author:)) > a")
			.map(|els| els.filter_map(|el| el.text()).collect());
		manga.url = Some(url.into());
	}

	fn parse_chapter_element(
		&self,
		params: &Params,
		element: &Element,
		manga: &Manga,
	) -> Option<Chapter> {
		let url = element.attr("abs:href")?;
		let name = element
			.select_first(".text-brand-400")
			.and_then(|el| el.text())
			.or_else(|| element.text())?;
		let title = helpers::clean_chapter_name(
			&manga.title,
			&params.chapter_name_prefix,
			&params.chapter_string,
			&name,
		);
		Some(Chapter {
			key: url
				.strip_prefix(&format!(
					"{}/{}/{}/",
					params.base_url, params.item_path, manga.key
				))?
				.into(),
			chapter_number: helpers::chapter_number(&title),
			title: Some(title),
			date_uploaded: element
				.select_first(".text-slate-500")
				.and_then(|el| el.text())
				.and_then(|date| parse_date(&date, &params.date_format)),
			url: Some(url),
			..Default::default()
		})
	}

	fn parse_page_list(&self, _params: &Params, html: &Document) -> Vec<Page> {
		html.select("#reader-all img")
			.map(|els| {
				els.filter_map(|img| {
					Some(Page {
						content: PageContent::url(img.img_attr()?),
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default()
	}

	fn get_search_manga_list(
		&self,
		params: &Params,
		query: Option<aidoku::alloc::string::String>,
		page: i32,
		filters: Vec<aidoku::FilterValue>,
	) -> Result<MangaPageResult> {
		let mut status_id: Option<String> = None;
		let mut type_id: Option<String> = None;
		let mut category: Option<String> = None;

		for filter in filters {
			if let FilterValue::Select { id, value } = filter {
				match id.as_str() {
					"status_id" => status_id = Some(value),
					"type_id" => type_id = Some(value),
					"category" => category = Some(value),
					_ => {}
				}
			}
		}

		let filters_page = self
			.modify_request(
				params,
				Request::get(format!("{}/advanced-search", params.base_url))?
					.header("Referer", &format!("{}/", params.base_url)),
			)?
			.html()?;
		let token = filters_page
			.select_first("input[name=_token]")
			.and_then(|el| el.attr("value"));
		let mut body = format!(
			"name={}&status_id={}&type_id={}&category={}&page={page}",
			query.map(encode_uri_component).unwrap_or_default(),
			status_id.as_deref().unwrap_or_default(),
			type_id.as_deref().unwrap_or_default(),
			category.as_deref().unwrap_or_default()
		);
		if let Some(token) = token {
			body.push_str(&format!("&_token={}", encode_uri_component(&token)));
		}
		let html = self
			.modify_request(
				params,
				Request::post(format!("{}/advanced-search", params.base_url))?
					.header("Content-Type", "application/x-www-form-urlencoded")
					.header("Referer", &format!("{}/advanced-search", params.base_url))
					.body(body),
			)?
			.html()?;
		Ok(self.parse_manga_page(params, &html))
	}

	fn get_manga_list(
		&self,
		params: &Params,
		_listing: aidoku::Listing,
		page: i32,
	) -> Result<MangaPageResult> {
		let html = Request::get(format!("{}/latest-release?page={page}", params.base_url))?
			.header("Referer", &format!("{}/", params.base_url))
			.html()?;
		Ok(MangaPageResult {
			entries: html
				.select("main > div > div.grid")
				.map(|els| {
					els.filter_map(|element| {
						let anchor = element.select_first("a.text-brand-400")?;
						let key: String = anchor
							.attr("abs:href")?
							.strip_prefix(&format!("{}/{}/", params.base_url, params.item_path))?
							.into();
						let cover = helpers::guess_cover(
							&params.base_url,
							&params.item_path,
							&key,
							element.select_first("img").and_then(|img| img.attr("src")),
						);
						Some(Manga {
							key,
							title: anchor.text()?,
							cover: Some(cover),
							..Default::default()
						})
					})
					.collect()
				})
				.unwrap_or_default(),
			has_next_page: html.select("nav span a[rel=next]").is_some(),
		})
	}

	fn get_home(&self, params: &Params) -> Result<HomeLayout> {
		let html = Request::get(BASE_URL)?.html()?;

		let id_prefix = format!("{BASE_URL}/{}/", params.item_path);

		let components = vec![
			HomeComponent {
				title: Some("Hot Comics".into()),
				subtitle: None,
				value: HomeComponentValue::Scroller {
					entries: html
						.select("#hot-track > a.hot-item")
						.map(|els| {
							els.filter_map(|el| {
								Some(
									Manga {
										key: el.attr("href")?.strip_prefix(&id_prefix)?.into(),
										title: el.select_first("p")?.text()?,
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
			},
			HomeComponent {
				title: Some("Latest Releases".into()),
				subtitle: None,
				value: HomeComponentValue::MangaChapterList {
					page_size: None,
					entries: html
						.select("section:has(h2:contains(Latest Releases)) > .grid > div")
						.map(|els| {
							els.filter_map(|el| {
								let anchor = el.select_first("a.text-brand-400")?;
								let manga = Manga {
									key: anchor.attr("href")?.strip_prefix(&id_prefix)?.into(),
									title: anchor.text()?,
									cover: el.select_first("img")?.attr("src"),
									..Default::default()
								};
								let chapter = Chapter {
									title: el.select_first("a.inline-flex")?.text(),
									..Default::default()
								};
								Some(MangaWithChapter { manga, chapter })
							})
							.collect()
						})
						.unwrap_or_default(),
					listing: Some(Listing {
						id: "Latest Releases".into(),
						name: "Latest Releases".into(),
						..Default::default()
					}),
				},
			},
		];

		Ok(HomeLayout { components })
	}
}

register_source!(
	MMRCMS<ReadComicsOnline>,
	Home,
	ListingProvider,
	ImageRequestProvider,
	DeepLinkHandler
);
