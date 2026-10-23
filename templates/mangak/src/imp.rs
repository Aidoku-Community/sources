use crate::{Params, helpers::*, models::*};
use aidoku::{
	Chapter, DeepLinkResult, FilterValue, HomeComponent, HomeComponentValue, HomeLayout, Link,
	Listing, Manga, MangaPageResult, Page, PageContent, PageContext, Result,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::{net::Request, std::send_partial_result},
	prelude::*,
};

pub trait Impl {
	fn new() -> Self;

	fn params(&self) -> Params;

	fn search_request(
		&self,
		params: &Params,
		mut qs: QueryParameters,
		page: i32,
	) -> Result<Request> {
		qs.push("page", Some(&page.to_string()));
		Ok(Request::get(format!(
			"{}/titles/search?{qs}",
			params.api_url
		))?)
	}

	fn search(&self, params: &Params, qs: QueryParameters, page: i32) -> Result<MangaPageResult> {
		let response: ApiResponse<SearchData> =
			self.search_request(params, qs, page)?.json_owned()?;
		Ok(MangaPageResult {
			entries: response
				.data
				.items
				.into_iter()
				.map(|item| item.into_manga(&params.base_url))
				.collect(),
			has_next_page: response.data.pagination.has_next,
		})
	}

	fn get_search_manga_list(
		&self,
		params: &Params,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut qs = QueryParameters::new();
		if let Some(query) = query {
			qs.push("q", Some(&query));
		}
		for filter in filters {
			match filter {
				FilterValue::Sort { index, .. } => {
					qs.push("sort", SORTS.get(index as usize).copied());
				}
				FilterValue::MultiSelect { id, included, .. } => {
					qs.push(&id, Some(&included.join(",")));
				}
				FilterValue::Check { id, value } if value != 0 => {
					qs.push(&id, Some("true"));
				}
				_ => {}
			}
		}
		self.search(params, qs, page)
	}

	fn get_manga_list(
		&self,
		params: &Params,
		listing: Listing,
		page: i32,
	) -> Result<MangaPageResult> {
		self.search(params, listing_query(&listing.id), page)
	}

	fn get_manga_update(
		&self,
		params: &Params,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let page: MangaPage = next_data(&format!("{}{}", params.base_url, manga.key))?;
		let details = page.initial_manga;
		let id = details.id.clone();

		if needs_details {
			manga.copy_from(details.into_manga(&params.base_url, manga.key.clone()));
			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let response: ApiResponse<ChapterList> =
				Request::get(format!("{}/titles/{id}/chapters", params.api_url))?.json_owned()?;
			manga.chapters = Some(
				response
					.data
					.chapters
					.into_iter()
					.map(|chapter| chapter.into_chapter(&params.base_url))
					.collect(),
			);
		}

		Ok(manga)
	}

	fn get_page_list(&self, params: &Params, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let page: ChapterPage = next_data(&format!("{}{}", params.base_url, chapter.key))?;
		Ok(page
			.initial_chapter
			.images
			.into_iter()
			.map(|url| Page {
				content: PageContent::url(url),
				..Default::default()
			})
			.collect())
	}

	fn get_home(&self, params: &Params) -> Result<HomeLayout> {
		let requests = ["today", "week", "latest"]
			.into_iter()
			.map(|id| self.search_request(params, listing_query(id), 1))
			.collect::<Result<Vec<_>>>()?;
		let mut sections = Request::send_all(requests)
			.into_iter()
			.map(|response| {
				Ok(response?
					.get_json_owned::<ApiResponse<SearchData>>()?
					.data
					.items)
			})
			.collect::<Result<Vec<_>>>()?
			.into_iter();
		let mut next_section = || sections.next().unwrap_or_default();
		let manga_links = |items: Vec<SearchItem>| -> Vec<Link> {
			items
				.into_iter()
				.map(|item| item.into_manga(&params.base_url).into())
				.collect()
		};

		Ok(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some("Popular Today".into()),
					subtitle: None,
					value: HomeComponentValue::Scroller {
						entries: manga_links(next_section()),
						listing: listing("today", "Popular Today"),
					},
				},
				HomeComponent {
					title: Some("Popular This Week".into()),
					subtitle: None,
					value: HomeComponentValue::MangaList {
						ranking: true,
						page_size: Some(10),
						entries: manga_links(next_section()),
						listing: listing("week", "Popular This Week"),
					},
				},
				HomeComponent {
					title: Some("Latest Updates".into()),
					subtitle: None,
					value: HomeComponentValue::MangaChapterList {
						page_size: Some(10),
						entries: next_section()
							.into_iter()
							.filter_map(|item| item.into_manga_with_chapter(&params.base_url))
							.collect(),
						listing: listing("latest", "Latest"),
					},
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
		let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
		Ok(match parts[..] {
			[manga] => Some(DeepLinkResult::Manga {
				key: format!("/{manga}"),
			}),
			[manga, chapter] => Some(DeepLinkResult::Chapter {
				manga_key: format!("/{manga}"),
				key: format!("/{manga}/{chapter}"),
			}),
			_ => None,
		})
	}
}
