use aidoku::{
	AidokuError, Chapter, FilterValue, HomeLayout, Listing, Manga, MangaPageResult, Page,
	PageContext, Result,
	alloc::{String, Vec, string::ToString, vec},
	imports::{net::Request, std::send_partial_result},
};

use crate::{
	auth::{AuthRequest, clear_user_id, get_user_id},
	chapters::get_chapters_cache,
	endpoints::Url,
	filters::FilterProcessor,
	home,
	models::{
		chapter::LibGroupChapterListItem,
		responses::{
			ChapterResponse, ChaptersResponse, MangaCoversResponse, MangaDetailResponse,
			MangaListResponse,
		},
	},
	settings::{get_api_url, get_base_url, get_cover_quality_url},
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
		let base_url = get_base_url();
		let cover_quality = get_cover_quality_url();

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
			.authed()?
			.get_json::<MangaListResponse>()?;

		let entries: Vec<Manga> = response
			.data
			.into_iter()
			.map(|manga_lib_manga| manga_lib_manga.into_manga(base_url.as_str(), &cover_quality))
			.collect();

		let has_next_page = response.meta.has_next_page.unwrap_or_default();

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn get_manga_update(
		&self,
		_params: &Params,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let api_url = get_api_url();
		let base_url = get_base_url();
		let user_id = get_user_id();
		let cover_quality = get_cover_quality_url();
		let slug_url = manga.key.clone();

		if needs_details {
			let details_url = Url::manga_details_with_fields(
				&api_url,
				&slug_url,
				&[
					"summary",
					"tags",
					"authors",
					"artists",
					"otherNames",
					"rate_avg",
				],
			);
			manga.copy_from(
				Request::get(details_url)?
					.authed()?
					.get_json::<MangaDetailResponse>()?
					.data
					.into_manga(base_url.as_str(), &cover_quality),
			);

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let chapters_url = Url::manga_chapters(api_url.as_str(), &slug_url);

			let chapters = LibGroupChapterListItem::flatten_chapters(
				Request::get(chapters_url)?
					.authed()?
					.get_json::<ChaptersResponse>()?
					.data,
				base_url.as_str(),
				&slug_url,
				&user_id,
			);

			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, params: &Params, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let api_url = get_api_url();
		let slug_url = manga.key.as_str();

		let chapter_number = chapter.chapter_number.unwrap_or_default();
		let volume = chapter.volume_number.unwrap_or_default();

		let chapters = get_chapters_cache(Some(3600)).get_chapters(slug_url, api_url.as_str())?;
		let branch_id: Option<i32> = chapters
			.into_iter()
			.flat_map(|c| c.branches)
			.find(|branch| branch.id.to_string() == chapter.key)
			.and_then(|branch| branch.branch_id);

		let pages_url =
			Url::chapter_pages_with_params(&api_url, slug_url, branch_id, chapter_number, volume);

		let pages = Request::get(pages_url)?
			.authed()?
			.get_json::<ChapterResponse>()?
			.data
			.into_pages(&params.site_id);

		Ok(pages)
	}

	fn get_home(&self, params: &Params) -> Result<HomeLayout> {
		// Initialize common variables
		let api_url = get_api_url();
		let site_id = &params.site_id;
		let site_id_str = site_id.to_string();
		let base_url = get_base_url();
		let cover_quality = get_cover_quality_url();

		// Send initial layout structure
		home::send_initial_layout();

		// Load popular manga (big scroller)
		home::load_popular_manga(&api_url, &site_id_str, base_url.as_str(), &cover_quality)?;

		// Load currently reading manga (daily trending)
		home::load_currently_reading(
			&api_url,
			&site_id_str,
			base_url.as_str(),
			&cover_quality,
			USER_AGENT,
		)?;

		// Load latest updates
		home::load_latest_updates(&api_url, &site_id_str, base_url.as_str(), &cover_quality)?;

		Ok(HomeLayout::default())
	}

	fn get_manga_list(
		&self,
		params: &Params,
		listing: Listing,
		page: i32,
	) -> Result<MangaPageResult> {
		let api_url = get_api_url();
		let site_id = &params.site_id;
		let site_id_str = site_id.to_string();
		let base_url = get_base_url();
		let cover_quality = get_cover_quality_url();
		let page_str = page.to_string();

		match listing.id.as_str() {
			"popular" => {
				// Popular manga
				let popular_params: Vec<(&str, &str)> = vec![
					("page", page_str.as_str()),
					("site_id[]", site_id_str.as_str()),
				];

				let popular_url = Url::manga_search_with_params(&api_url, &popular_params);

				let response = Request::get(popular_url)?
					.authed()?
					.get_json::<MangaListResponse>()?;

				let entries: Vec<Manga> = response
					.data
					.into_iter()
					.map(|manga_lib_manga| {
						manga_lib_manga.into_manga(base_url.as_str(), &cover_quality)
					})
					.collect();

				let has_next_page = response.meta.has_next_page.unwrap_or_default();

				Ok(MangaPageResult {
					entries,
					has_next_page,
				})
			}
			"currently_reading" => {
				// "Сейчас читают" with configurable parameters
				let params: Vec<(&str, &str)> = vec![
					("page", page_str.as_str()),
					("popularity", "1"), // Default to "Новинки"
					("time", "day"),     // Default to "За день"
					("site_id[]", site_id_str.as_str()),
				];

				let currently_reading_url = Url::top_views_with_params(&api_url, &params);

				let response = Request::get(currently_reading_url)?
					.header("Referer", &api_url)
					.header("Site-Id", &site_id.to_string())
					.header("User-Agent", USER_AGENT)
					.authed()?
					.get_json::<MangaListResponse>()?;

				let entries: Vec<Manga> = response
					.data
					.into_iter()
					.map(|manga_lib_manga| {
						manga_lib_manga.into_manga(base_url.as_str(), &cover_quality)
					})
					.collect();

				let has_next_page = response.meta.has_next_page.unwrap_or_default();

				Ok(MangaPageResult {
					entries,
					has_next_page,
				})
			}
			"latest" => {
				// Latest updates
				let latest_params: Vec<(&str, &str)> = vec![
					("page", page_str.as_str()),
					("site_id[]", site_id_str.as_str()),
					("sort_by", "last_chapter_at"),
				];

				let latest_url = Url::manga_search_with_params(&api_url, &latest_params);

				let response = Request::get(latest_url)?
					.authed()?
					.get_json::<MangaListResponse>()?;

				let entries: Vec<Manga> = response
					.data
					.into_iter()
					.map(|manga_lib_manga| {
						manga_lib_manga.into_manga(base_url.as_str(), &cover_quality)
					})
					.collect();

				let has_next_page = response.meta.has_next_page.unwrap_or_default();

				Ok(MangaPageResult {
					entries,
					has_next_page,
				})
			}
			_ => Err(AidokuError::message("Unknown listing")),
		}
	}

	fn get_image_request(
		&self,
		params: &Params,
		url: String,
		_context: Option<PageContext>,
	) -> Result<Request> {
		let api_url = get_api_url();
		let site_id = &params.site_id;

		Ok(Request::get(url)?
			.header("Referer", &api_url)
			.header("Site-Id", &site_id.to_string())
			.header("User-Agent", USER_AGENT))
	}

	fn get_alternate_covers(&self, _params: &Params, manga: Manga) -> Result<Vec<String>> {
		let api_url = get_api_url();
		let cover_quality = get_cover_quality_url();

		let covers_url = Url::manga_covers(&api_url, &manga.key);

		Ok(Request::get(covers_url)?
			.authed()?
			.get_json::<MangaCoversResponse>()?
			.data
			.iter()
			.map(|c| c.cover.get_cover_url(&cover_quality))
			.collect())
	}

	fn handle_manga_migration(&self, _params: &Params, key: String) -> Result<String> {
		Ok(key)
	}

	fn handle_chapter_migration(
		&self,
		_params: &Params,
		manga_key: String,
		chapter_key: String,
	) -> Result<String> {
		let base_url = get_base_url();
		let parts: Vec<&str> = chapter_key.split('#').collect();
		if parts.len() != 2 {
			// Return original if invalid format
			return Ok(chapter_key);
		}

		let (chapter_number, volume_number) = (parts[0], parts[1]);

		let chapters = get_chapters_cache(None).get_chapters(&manga_key, base_url.as_str())?;

		let chapter = chapters
			.iter()
			.find(|c| c.number == chapter_number && c.volume == volume_number)
			.ok_or_else(|| AidokuError::message("Chapter not found"))?;

		let branch_id = chapter
			.branches
			.first()
			.map(|b| b.id)
			.ok_or_else(|| AidokuError::message("No branch ID found"))?;

		Ok(branch_id.to_string())
	}

	fn handle_notification(&self, _params: &Params, notification: String) {
		match notification.as_str() {
			"system.endMigration" => {
				get_chapters_cache(None).clear();
			}
			"token.changed" => {
				clear_user_id();
				get_user_id();
			}
			_ => {}
		}
	}
}
