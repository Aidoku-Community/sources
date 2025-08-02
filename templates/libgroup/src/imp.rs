use aidoku::{
	Chapter, DeepLinkResult, FilterValue, HomeComponent, HomeComponentValue, HomeLayout,
	HomePartialResult, Link, Listing, ListingKind, Manga, MangaPageResult, Page, PageContext,
	Result,
	alloc::{String, Vec, string::ToString, vec},
	imports::{net::Request, std::send_partial_result},
};

use crate::{
	endpoints::Url,
	filters::FilterProcessor,
	models::{
		chapter::LibGroupChapterListItem,
		responses::{ChapterResponse, ChaptersResponse, MangaDetailResponse, MangaListResponse},
	},
	settings::get_api_url,
};

use super::Params;

static USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1";

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
		let api_url = get_api_url();
		let site_id = &params.site_id;
		let base_url = &params.base_url;

		let mut query_params = Vec::new();

		if let Some(q) = query
			&& !q.trim().is_empty()
		{
			query_params.push(("q", q));
		}

		query_params.push(("page", page.to_string()));
		query_params.push(("site_id[]", site_id.to_string()));

		let filter_processor = FilterProcessor::new();
		query_params.extend(filter_processor.process_filters(filters));

		let params_for_url: Vec<(&str, &str)> =
			query_params.iter().map(|(k, v)| (*k, v.as_str())).collect();

		let search_url = Url::manga_search_with_params(&api_url, &params_for_url);

		let response = Request::get(search_url)?
			.send()?
			.get_json::<MangaListResponse>()?;

		let entries: Vec<Manga> = response
			.data
			.into_iter()
			.map(|manga_lib_manga| manga_lib_manga.into_manga(base_url))
			.collect();

		let has_next_page = response.meta.has_next_page.unwrap_or_default();

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
		let api_url = get_api_url();
		let base_url = &params.base_url;
		let manga_slug = manga.key.clone();

		if needs_details {
			let details_url = Url::manga_details_with_fields(
				&api_url,
				&manga_slug,
				&["summary", "tags", "authors", "artists"],
			);
			manga.copy_from(
				Request::get(details_url)?
					.send()?
					.get_json::<MangaDetailResponse>()?
					.data
					.into_manga(base_url),
			);

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let chapters_url = Url::MangaChapters { slug: &manga_slug }.build(&api_url);

			let chapters = LibGroupChapterListItem::flatten_chapters(
				Request::get(chapters_url)?
					.send()?
					.get_json::<ChaptersResponse>()?
					.data,
				base_url,
				&manga_slug,
			);

			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, params: &Params, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let api_url = get_api_url();
		let manga_slug = manga.key.as_str();

		let chapter_number = chapter.chapter_number.unwrap_or_default();
		let volume = chapter.volume_number.unwrap_or_default();
		let branch_id = None;

		let pages_url =
			Url::chapter_pages_with_params(&api_url, manga_slug, branch_id, chapter_number, volume);

		let pages = Request::get(pages_url)?
			.send()?
			.get_json::<ChapterResponse>()?
			.data
			.into_pages(&params.site_id);

		Ok(pages)
	}

	fn get_home(&self, params: &Params) -> Result<HomeLayout> {
		let api_url = get_api_url();
		let site_id = &params.site_id;
		let base_url = &params.base_url;

		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: vec![HomeComponent {
				title: Some("Популярное".into()),
				subtitle: None,
				value: HomeComponentValue::empty_scroller(),
			}],
		}));

		let popular_url =
			Url::manga_search_with_params(&api_url, &[("site_id[]", &site_id.to_string())]);

		let response = Request::get(popular_url)?
			.send()?
			.get_json::<MangaListResponse>()?;

		let manga_entries: Vec<Link> = response
			.data
			.into_iter()
			.map(|manga_lib_manga| {
				let manga: Manga = manga_lib_manga.into_manga(base_url);
				Link::from(manga)
			})
			.collect();

		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some("Популярное".into()),
			subtitle: None,
			value: HomeComponentValue::Scroller {
				entries: manga_entries,
				listing: Some(Listing {
					id: String::from("popular"),
					name: String::from("Популярное"),
					kind: ListingKind::Default,
				}),
			},
		}));

		Ok(HomeLayout::default())
	}

	fn get_manga_list(
		&self,
		params: &Params,
		listing: Listing,
		page: i32,
	) -> Result<MangaPageResult> {
		let api_url = get_api_url();
		let base_url = &params.base_url;
		let page_str = page.to_string();

		let listing_params = match listing.name.as_str() {
			"popular" => vec![("sort", "popular"), ("page", page_str.as_str())],
			"latest" => vec![("sort", "updated"), ("page", page_str.as_str())],
			"trending" => vec![("sort", "trending"), ("page", page_str.as_str())],
			_ => vec![("page", page_str.as_str())],
		};

		let listing_url = Url::manga_search_with_params(&api_url, &listing_params);
		let response = Request::get(listing_url)?
			.send()?
			.get_json::<MangaListResponse>()?;

		let entries: Vec<Manga> = response
			.data
			.into_iter()
			.map(|manga_lib_manga| manga_lib_manga.into_manga(base_url))
			.collect();

		let has_next_page = response.meta.has_next_page.unwrap_or_default();

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn get_image_request(
		&self,
		_params: &Params,
		url: String,
		_context: Option<PageContext>,
	) -> Result<Request> {
		let api_url = get_api_url();

		Ok(Request::get(url)?
			.header("Referer", &api_url)
			.header("User-Agent", USER_AGENT))
	}

	fn handle_deep_link(&self, _params: &Params, _url: String) -> Result<Option<DeepLinkResult>> {
		Ok(None)
	}
}
