use crate::{Params, models::*, rsc};
use aidoku::{
	Chapter, DeepLinkResult, FilterValue, HomeComponent, HomeComponentValue, HomeLayout, Listing,
	Manga, MangaPageResult, Page, PageContent, PageContext, Result,
	alloc::{String, Vec, format, string::ToString},
	helpers::uri::QueryParameters,
	imports::{html::Element, net::Request, std::send_partial_result},
};

pub trait Impl {
	fn new() -> Self;

	fn params(&self) -> Params;

	fn get_search_manga_list(
		&self,
		params: &Params,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		self.api_page(params, page, "", query.as_deref(), &filters)
	}

	fn get_manga_list(
		&self,
		params: &Params,
		listing: Listing,
		page: i32,
	) -> Result<MangaPageResult> {
		self.api_page(params, page, &listing.id, None, &[])
	}

	fn get_manga_update(
		&self,
		params: &Params,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{}/series/comic/{}?sort=desc", params.base_url, manga.key);
		let details: Detail = rsc::extract(&self.rsc_get(&url)?, "series")?;

		if needs_details {
			manga.copy_from(details.series.into_manga(&params.base_url));
			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let mut chapter_data = details.chapters;
			for page in 2..=details.total_pages {
				let next: Detail =
					rsc::extract(&self.rsc_get(&format!("{url}&page={page}"))?, "series")?;
				chapter_data.extend(next.chapters);
			}
			let chapters: Vec<Chapter> = chapter_data
				.into_iter()
				.filter(|chapter| !chapter.is_locked)
				.map(|chapter| chapter.into_chapter(&params.base_url, &manga.key))
				.collect();
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, params: &Params, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!(
			"{}/series/comic/{}/chapter/{}",
			params.base_url, manga.key, chapter.key
		);
		let detail: ChapterDetail = rsc::extract(&self.rsc_get(&url)?, "chapter")?;
		Ok(detail
			.chapter
			.pages
			.into_iter()
			.filter_map(|page| page.image_url)
			.map(|url| Page {
				content: PageContent::url(absolute_url(&params.base_url, &url)),
				..Default::default()
			})
			.collect())
	}

	fn get_image_request(
		&self,
		params: &Params,
		url: String,
		_context: Option<PageContext>,
	) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", &format!("{}/", params.base_url)))
	}

	fn get_home(&self, params: &Params) -> Result<HomeLayout> {
		use crate::home::*;

		let html: Element = Request::get(&params.base_url)?.html()?.into();
		let mut components = Vec::new();

		if let Some(section) = home_section(&html, "homepage_latest") {
			let entries = latest_entries(&section, &params.base_url);
			if !entries.is_empty() {
				components.push(HomeComponent {
					title: section_title(&section),
					subtitle: None,
					value: HomeComponentValue::MangaChapterList {
						entries,
						page_size: Some(3),
						listing: None,
					},
				});
			}
		}

		if let Some(section) = home_section(&html, "homepage_new") {
			let entries = home_entries_in(&section, "homepage_new", &params.base_url);
			if !entries.is_empty() {
				components.push(HomeComponent {
					title: section_title(&section),
					subtitle: None,
					value: HomeComponentValue::Scroller {
						entries: entries.into_iter().map(Into::into).collect(),
						listing: None,
					},
				});
			}
		}

		if let Some(section) = home_section(&html, "homepage_popular") {
			let entries = home_entries_in(&section, "homepage_popular", &params.base_url);
			if !entries.is_empty() {
				components.push(HomeComponent {
					title: section_title(&section),
					subtitle: None,
					value: HomeComponentValue::Scroller {
						entries: entries.into_iter().map(Into::into).collect(),
						listing: None,
					},
				});
			}
		}

		if let Some(section) = home_section(&html, "homepage_most_popular") {
			let entries = home_entries_in(&section, "homepage_most_popular", &params.base_url);
			if !entries.is_empty() {
				components.push(HomeComponent {
					title: section_title(&section),
					subtitle: None,
					value: HomeComponentValue::MangaList {
						ranking: true,
						page_size: Some(3),
						entries: entries.into_iter().map(Into::into).collect(),
						listing: None,
					},
				});
			}
		}

		Ok(HomeLayout { components })
	}

	fn handle_deep_link(&self, params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
		let prefix = format!("{}/series/comic/", params.base_url);
		let Some(rest) = url.strip_prefix(&prefix) else {
			return Ok(None);
		};
		let slug = rest.split(['?', '#', '/']).next().unwrap_or_default();
		if slug.is_empty() {
			return Ok(None);
		}
		if let Some(chapter) = rest.strip_prefix(&format!("{slug}/chapter/")) {
			let chapter = chapter.split(['?', '#', '/']).next().unwrap_or_default();
			if chapter.is_empty() {
				return Ok(None);
			}
			return Ok(Some(DeepLinkResult::Chapter {
				manga_key: slug.into(),
				key: chapter.into(),
			}));
		}
		Ok(Some(DeepLinkResult::Manga { key: slug.into() }))
	}

	fn api_page(
		&self,
		params: &Params,
		page: i32,
		sort: &str,
		query: Option<&str>,
		filters: &[FilterValue],
	) -> Result<MangaPageResult> {
		let mut qs = QueryParameters::new();
		qs.push_encoded("limit", Some("24"));
		qs.push_encoded("contentMode", Some("comics"));
		qs.push_encoded("page", Some(&page.to_string()));
		if !sort.is_empty() {
			qs.push("sort", Some(sort));
		}
		if query.is_some() {
			qs.push("q", query);
		}

		for filter in filters {
			match filter {
				FilterValue::Sort { index, .. } => {
					let sort = match index {
						0 => "updated",
						1 => "popular",
						2 => "trending",
						3 => "views",
						4 => "rating",
						5 => "longest",
						6 => "newest",
						_ => "updated",
					};
					qs.set("sort", Some(sort));
				}
				FilterValue::Select { id, value } if !value.is_empty() => qs.set(id, Some(value)),
				FilterValue::MultiSelect { id, included, .. } => {
					qs.set(id, Some(&included.join(",")))
				}
				_ => {}
			}
		}

		let response: SeriesResponse =
			Request::get(format!("{}/api/series?{qs}", params.base_url))?.json_owned()?;
		Ok(MangaPageResult {
			entries: response
				.data
				.into_iter()
				.map(|series| series.into_basic_manga(&params.base_url))
				.collect(),
			has_next_page: response.meta.has_more,
		})
	}

	fn rsc_get(&self, url: &str) -> Result<String> {
		Request::get(url)?.header("rsc", "1").string()
	}
}
