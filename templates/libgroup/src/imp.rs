use aidoku::{
	AidokuError, Chapter, FilterValue, HomeLayout, Listing, Manga, MangaPageResult, Page,
	PageContext, Result,
	alloc::{String, Vec, string::ToString, vec},
	imports::{net::Request, std::send_partial_result},
};

use crate::{
	auth::{AuthRequest, USER_AGENT, clear_user_id, get_user_id},
	chapters::get_chapters_cache,
	context::Context,
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
};

use super::Params;

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
		let ctx = Context::from_params(params);
		let mut query_params = Vec::new();

		if let Some(q) = query
			&& !q.trim().is_empty()
		{
			query_params.push(("q", q));
		}

		query_params.push(("page", page.to_string()));
		query_params.push(("site_id[]", ctx.site_id.to_string()));

		let filter_processor = FilterProcessor::new();
		query_params.extend(filter_processor.process_filters(filters));

		let params_for_url: Vec<(&str, &str)> =
			query_params.iter().map(|(k, v)| (*k, v.as_str())).collect();

		let search_url = Url::manga_search_with_params(&ctx.api_url, &params_for_url);

		let response = Request::get(search_url)?
			.authed(&ctx)?
			.get_json::<MangaListResponse>()?;

		let entries: Vec<Manga> = response
			.data
			.into_iter()
			.map(|manga_lib_manga| manga_lib_manga.into_manga(&ctx))
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
		let ctx = Context::from_params(params);
		let user_id = get_user_id(&ctx);
		let slug_url = manga.key.clone();

		if needs_details {
			let details_url = Url::manga_details_with_fields(
				&ctx.api_url,
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
					.authed(&ctx)?
					.get_json::<MangaDetailResponse>()?
					.data
					.into_manga(&ctx),
			);

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let chapters_url = Url::manga_chapters(ctx.api_url.as_str(), &slug_url);

			let chapters = LibGroupChapterListItem::flatten_chapters(
				Request::get(chapters_url)?
					.authed(&ctx)?
					.get_json::<ChaptersResponse>()?
					.data,
				ctx.base_url.as_str(),
				&slug_url,
				&user_id,
			);

			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, params: &Params, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let ctx = Context::from_params(params);
		let slug_url = manga.key.as_str();

		let chapter_number = chapter.chapter_number.unwrap_or_default();
		let volume = chapter.volume_number.unwrap_or_default();

		let chapters = get_chapters_cache(Some(3600)).get_chapters(slug_url, &ctx)?;
		let branch_id: Option<i32> = chapters
			.into_iter()
			.flat_map(|c| c.branches)
			.find(|branch| branch.id.to_string() == chapter.key)
			.and_then(|branch| branch.branch_id);

		let pages_url = Url::chapter_pages_with_params(
			&ctx.api_url,
			slug_url,
			branch_id,
			chapter_number,
			volume,
		);

		let pages = Request::get(pages_url)?
			.authed(&ctx)?
			.get_json::<ChapterResponse>()?
			.data
			.ok_or_else(|| AidokuError::message("Chapter is empty"))?
			.into_pages(&ctx);

		Ok(pages)
	}

	fn get_home(&self, params: &Params) -> Result<HomeLayout> {
		let ctx = Context::from_params(params);

		home::send_initial_layout();
		home::load_popular_manga(&ctx)?;
		home::load_currently_reading(&ctx)?;
		home::load_latest_updates(&ctx)?;

		Ok(HomeLayout::default())
	}

	fn get_manga_list(
		&self,
		params: &Params,
		listing: Listing,
		page: i32,
	) -> Result<MangaPageResult> {
		let ctx = Context::from_params(params);
		let site_id_str = ctx.site_id.to_string();
		let page_str = page.to_string();

		match listing.id.as_str() {
			"popular" => {
				// Popular manga
				let popular_params: Vec<(&str, &str)> =
					vec![("page", page_str.as_str()), ("site_id[]", &site_id_str)];

				let popular_url = Url::manga_search_with_params(&ctx.api_url, &popular_params);

				let response = Request::get(popular_url)?
					.authed(&ctx)?
					.get_json::<MangaListResponse>()?;

				let entries: Vec<Manga> = response
					.data
					.into_iter()
					.map(|manga_lib_manga| manga_lib_manga.into_manga(&ctx))
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
					("site_id[]", &site_id_str),
				];

				let currently_reading_url = Url::top_views_with_params(&ctx.api_url, &params);

				let response = Request::get(currently_reading_url)?
					.authed(&ctx)?
					.get_json::<MangaListResponse>()?;

				let entries: Vec<Manga> = response
					.data
					.into_iter()
					.map(|manga_lib_manga| manga_lib_manga.into_manga(&ctx))
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

				let latest_url = Url::manga_search_with_params(&ctx.api_url, &latest_params);

				let response = Request::get(latest_url)?
					.authed(&ctx)?
					.get_json::<MangaListResponse>()?;

				let entries: Vec<Manga> = response
					.data
					.into_iter()
					.map(|manga_lib_manga| manga_lib_manga.into_manga(&ctx))
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
		let ctx = Context::from_params(params);

		Ok(Request::get(url)?
			.header("Origin", &ctx.base_url)
			.header("Referer", &ctx.api_url)
			.header("Site-Id", &ctx.site_id.to_string())
			.header("User-Agent", USER_AGENT))
	}

	fn get_alternate_covers(&self, params: &Params, manga: Manga) -> Result<Vec<String>> {
		let ctx = Context::from_params(params);

		let covers_url = Url::manga_covers(&ctx.api_url, &manga.key);

		Ok(Request::get(covers_url)?
			.authed(&ctx)?
			.get_json::<MangaCoversResponse>()?
			.data
			.iter()
			.map(|c| c.cover.get_cover_url(&ctx.cover_quality))
			.collect())
	}

	fn handle_manga_migration(&self, _params: &Params, key: String) -> Result<String> {
		Ok(key)
	}

	fn handle_chapter_migration(
		&self,
		params: &Params,
		manga_key: String,
		chapter_key: String,
	) -> Result<String> {
		let ctx = Context::from_params(params);
		let parts: Vec<&str> = chapter_key.split('#').collect();
		if parts.len() != 2 {
			// Return original if invalid format
			return Ok(chapter_key);
		}

		let (chapter_number, volume_number) = (parts[0], parts[1]);

		let chapters = get_chapters_cache(None).get_chapters(&manga_key, &ctx)?;

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

	fn handle_notification(&self, params: &Params, notification: String) {
		let ctx = Context::from_params(params);

		match notification.as_str() {
			"system.endMigration" => {
				get_chapters_cache(None).clear();
			}
			"token.changed" => {
				clear_user_id();
				get_user_id(&ctx);
			}
			_ => {}
		}
	}
}
