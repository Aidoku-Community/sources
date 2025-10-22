use super::{auth, models::*, parser, AuthedRequest, Params};
use aidoku::{
	alloc::{string::ToString, String, Vec},
	helpers::uri::QueryParameters,
	imports::{
		canvas::{Canvas, ImageRef, Rect},
		error::AidokuError,
		net::Request,
		std::send_partial_result,
	},
	prelude::*,
	Chapter, DeepLinkResult, FilterValue, HomeLayout, ImageResponse, Listing, Manga,
	MangaPageResult, Page, PageContent, PageContext, Result, Viewer,
};

pub trait Impl {
	fn new() -> Self;

	fn params(&self) -> Params;

	fn get_manga_list(
		&self,
		_params: &Params,
		_listing: Listing,
		_page: i32,
	) -> Result<MangaPageResult> {
		Err(AidokuError::message("Invalid listing"))
	}

	fn get_search_manga_list(
		&self,
		params: &Params,
		query: Option<String>,
		_page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let Some(query) = query else {
			return Ok(MangaPageResult::default());
		};

		let mut qs = QueryParameters::new();
		qs.push("q", Some(&query));
		let url = format!("{}/search?{}", params.base_url, qs);
		let html = Request::get(url)?.authed().html()?;

		let entries = parser::parse_response(
			&html,
			&params.base_url,
			&params.search_item_selector,
			&params.search_item_title_selector,
			"img",
			"src",
			None,
			None,
		);

		Ok(MangaPageResult {
			entries,
			has_next_page: false,
		})
	}

	fn get_manga_update(
		&self,
		params: &Params,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let mut new_manga = manga.clone();

		let url = format!("{}{}", params.base_url, manga.key);
		let html = Request::get(&url)?.authed().html()?;

		if needs_details {
			let element = html
				.select_first("section.series-information div.series-header")
				.ok_or(AidokuError::message("漫画の情報がありません"))?;
			let title = element
				.select_first("h1.series-header-title")
				.and_then(|e| e.text())
				.unwrap_or(new_manga.title.clone());
			let cover = html
				.select_first("div.series-header-image-wrapper img")
				.and_then(|e| e.attr("data-src"));
			let authors = element
				.select_first("h2.series-header-author")
				.and_then(|e| {
					let text = e.text()?;
					Some(text.split('/').map(String::from).collect::<Vec<_>>())
				});
			let description = element
				.select_first("p.series-header-description")
				.and_then(|e| e.text());
			let is_scroll = html
				.select_first("#content")
				.map(|e| e.has_class("content-vertical")) // has content-horizontal normally
				.unwrap_or(false);

			new_manga.copy_from(Manga {
				key: new_manga.key.clone(),
				title,
				cover,
				authors,
				description,
				url: Some(url.clone()),
				viewer: if is_scroll {
					Viewer::Webtoon
				} else {
					Viewer::RightToLeft
				},
				..Default::default()
			});

			if needs_chapters {
				send_partial_result(&new_manga);
			}
		}

		if needs_chapters {
			let chapters = if params.is_paginated {
				parser::parse_chapters_paginated(&html, &params.base_url, &url)?
			} else {
				parser::parse_chapters_single_page(
					&html,
					&params.base_url,
					&url,
					&new_manga.title,
					&params.chapter_list_selector,
				)?
			};
			new_manga.chapters = Some(chapters);
		}

		Ok(new_manga)
	}

	fn get_page_list(
		&self,
		_params: &Params,
		_manga: Manga,
		chapter: Chapter,
	) -> Result<Vec<Page>> {
		let Some(url) = chapter.url else {
			return Err(AidokuError::message("URLがありません"));
		};
		let html = Request::get(url)?.authed().html()?;

		let episode = html
			.select_first("script#episode-json")
			.and_then(|e| e.attr("data-value"))
			.ok_or(AidokuError::message("このチャプターは非公開です"))
			.and_then(|v| {
				serde_json::from_str::<GigaEpisode>(v.as_ref()).map_err(AidokuError::JsonParseError)
			})?;

		Ok(episode
			.readable_product
			.page_structure
			.pages
			.iter()
			.filter_map(|page| {
				if page.r#type.as_ref().is_none_or(|t| t != "main") {
					return None;
				}
				let src = page.src.as_ref()?;
				let mut context = PageContext::new();
				context.insert(String::from("width"), page.width.unwrap_or(0).to_string());
				context.insert(String::from("height"), page.height.unwrap_or(0).to_string());
				Some(Page {
					content: PageContent::url_context(src, context),
					..Default::default()
				})
			})
			.collect())
	}

	fn process_page_image(
		&self,
		params: &Params,
		response: ImageResponse,
		context: Option<PageContext>,
	) -> Result<ImageRef> {
		let Some(context) = context else {
			return Err(AidokuError::message("Missing page context"));
		};

		// if the image is not from the specified CDN, return the original image without trying to descramble
		if response
			.request
			.url
			.is_none_or(|url| !url.starts_with(params.cdn_url.as_ref()))
		{
			return Ok(response.image);
		}

		let width = context
			.get("width")
			.and_then(|w| w.parse::<usize>().ok())
			.unwrap_or(0);
		let height = context
			.get("height")
			.and_then(|h| h.parse::<usize>().ok())
			.unwrap_or(0);

		const DIVIDE_NUM: i32 = 4;
		const MULTIPLE: i32 = 8;

		let c_width = ((width as f32) / (DIVIDE_NUM * MULTIPLE) as f32) as i32 * MULTIPLE;
		let c_height = ((height as f32) / (DIVIDE_NUM * MULTIPLE) as f32) as i32 * MULTIPLE;

		let mut canvas = Canvas::new(width as f32, height as f32);

		// first, copy the entire image to the canvas (since the edges sometimes aren't scrambled)
		let img_rect = Rect::new(0.0, 0.0, width as f32, height as f32);
		canvas.draw_image(&response.image, img_rect);

		for e in 0..DIVIDE_NUM * DIVIDE_NUM {
			let x = e % DIVIDE_NUM * c_width;
			let y = (e as f32 / DIVIDE_NUM as f32) as i32 * c_height;
			let cell_src = Rect::new(x as f32, y as f32, c_width as f32, c_height as f32);

			let row = (e as f32 / DIVIDE_NUM as f32) as i32;
			let dst_e = e % DIVIDE_NUM * DIVIDE_NUM + row;
			let dst_x = dst_e % DIVIDE_NUM * c_width;
			let dst_y = (dst_e as f32 / DIVIDE_NUM as f32) as i32 * c_height;
			let cell_dst = Rect::new(dst_x as f32, dst_y as f32, c_width as f32, c_height as f32);

			canvas.copy_image(&response.image, cell_src, cell_dst);
		}

		Ok(canvas.get_image())
	}

	fn get_home(&self, _params: &Params) -> Result<HomeLayout> {
		Err(AidokuError::Unimplemented)
	}

	fn handle_basic_login(
		&self,
		params: &Params,
		_key: String,
		username: String,
		password: String,
	) -> Result<bool> {
		auth::login(&params.base_url, &username, &password)
	}

	fn handle_notification(&self, _params: &Params, notification: String) {
		// handle log out
		if notification == "login" {
			let logged_in = auth::is_logged_in();
			if !logged_in {
				// if the username/password default keys were removed, we should remove the cookie key
				auth::logout();
			}
		}
	}

	fn handle_deep_link(&self, params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(params.base_url.as_ref()) else {
			return Ok(None);
		};

		const EPISODE_PATH: &str = "episode/";

		if let Some(key) = path.strip_prefix(EPISODE_PATH) {
			// ex: https://shonenjumpplus.com/episode/10834108156648240735
			// the manga key can be any of the chapter keys
			Ok(Some(DeepLinkResult::Chapter {
				manga_key: key.into(),
				key: key.into(),
			}))
		} else {
			Ok(None)
		}
	}
}
