#![no_std]
use aidoku::{
	BasicLoginHandler, Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, HashMap,
	ImageRequestProvider, ImageResponse, Listing, ListingProvider, Manga, MangaPageResult,
	NotificationHandler, Page, PageContent, PageContext, PageImageProcessor, Result, Source,
	alloc::{String, Vec, vec},
	canvas::Rect,
	imports::{
		canvas::{Canvas, ImageRef},
		net::Request,
	},
	prelude::*,
};

mod home;
mod models;
mod net;
mod settings;

use models::{AlbumResp, BlockState, ChapterResp, SearchResp};
use net::ApiContext;

const WEB_URL: &str = "https://18comic.vip";
const PAGE_SIZE: i32 = 80;
const SCRAMBLE_ID: u64 = 220980;

struct JMComic;

impl Source for JMComic {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let api = net::context()?;
		let block = block_ctx();
		let (order, category, keyword) = Self::parse_filters(query.as_deref(), &filters);

		if let Some(kw) = keyword {
			if page <= 1
				&& category.is_empty()
				&& let Some(key) = parse_manga_key(&api, kw)?
			{
				return finish_search_result(page, direct_manga_result(&api, &key, &block)?);
			}
			return finish_search_result(
				page,
				search_result(&api, &net::url::search(kw, order, category, page), &block)?,
			);
		}

		finish_search_result(
			page,
			search_result(&api, &net::url::filter(order, category, page), &block)?,
		)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let api = net::context()?;
		let resp = visible_album(&api, &manga.key, &block_ctx())?;

		if needs_chapters {
			manga.chapters = Some(resp.to_chapters(&manga.key));
		}
		if needs_details {
			manga.copy_from(resp.into_manga(&manga.key, &api.cdn_base));
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let api = net::context()?;
		let block = block_ctx();
		if !block.is_empty() {
			visible_album(&api, &manga.key, &block)?;
		}

		let resp: ChapterResp = api.get(&net::url::chapter(&chapter.key))?;
		let ep_id: String = resp.episode_id(&chapter.key).into();
		let cdn_base = &api.cdn_base;

		Ok(resp
			.images
			.into_iter()
			.map(|filename| {
				let url = format!("{cdn_base}/media/photos/{ep_id}/{filename}");
				let mut page_ctx = HashMap::new();
				page_ctx.insert("ep_id".into(), ep_id.clone());
				page_ctx.insert("filename".into(), filename);
				Page {
					content: PageContent::url_context(url, page_ctx),
					..Default::default()
				}
			})
			.collect())
	}
}

impl ListingProvider for JMComic {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let api = net::context()?;
		let block = block_ctx();
		match listing.id.as_str() {
			id if id.starts_with("promo:") => home::listing_page(&api, &id[6..], page, &block),
			id => search_result(&api, &Self::listing_url(id, page)?, &block),
		}
	}
}

impl PageImageProcessor for JMComic {
	fn process_page_image(
		&self,
		response: ImageResponse,
		context: Option<PageContext>,
	) -> Result<ImageRef> {
		let Some(ctx) = context.as_ref() else {
			return Ok(response.image);
		};

		let ep_id: u64 = ctx.get("ep_id").and_then(|s| s.parse().ok()).unwrap_or(0);
		let filename = ctx.get("filename").map(String::as_str).unwrap_or_default();

		if filename.ends_with(".gif") {
			return Ok(response.image);
		}

		let num = Self::calc_scramble_num(ep_id, filename);
		if num <= 1 {
			return Ok(response.image);
		}

		let w = response.image.width();
		let h = response.image.height();
		let h_px = h as u32;
		let block = h_px / num;
		let rem = h_px % num;

		let mut canvas = Canvas::new(w, h);
		let mut dst_y = 0.0f32;

		// reassemble slices in reverse order to restore the original image
		for i in (0..num as usize).rev() {
			let src_y = (i as u32 * block) as f32;
			let cur_h = if i == num as usize - 1 {
				(block + rem) as f32
			} else {
				block as f32
			};
			canvas.copy_image(
				&response.image,
				Rect::new(0.0, src_y, w, cur_h),
				Rect::new(0.0, dst_y, w, cur_h),
			);
			dst_y += cur_h;
		}

		Ok(canvas.get_image())
	}
}

impl DeepLinkHandler for JMComic {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		if let Some(key) = extract_id(&url, "/album/") {
			return Ok(Some(DeepLinkResult::Manga { key }));
		}
		if let Some(key) = extract_id(&url, "/photo/") {
			let api = net::context()?;
			let chapter: ChapterResp = api.get(&net::url::chapter(&key))?;
			return Ok(Some(DeepLinkResult::Chapter {
				manga_key: chapter.series_key(&key).into(),
				key,
			}));
		}
		Ok(None)
	}
}

impl ImageRequestProvider for JMComic {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?
			.header("referer", "https://localhost/")
			.header("user-agent", net::JM_UA)
			.header("x-requested-with", net::JM_PKG))
	}
}

impl BasicLoginHandler for JMComic {
	fn handle_basic_login(&self, key: String, username: String, password: String) -> Result<bool> {
		if key != "login" {
			bail!("登录入口无效");
		}
		if username.is_empty() || password.is_empty() {
			return Ok(false);
		}
		match net::login(&username, &password) {
			Ok(auth) => {
				settings::set_auth(&auth);
				settings::set_just_logged_in();
				Ok(true)
			}
			Err(_) => Ok(false),
		}
	}
}

impl NotificationHandler for JMComic {
	fn handle_notification(&self, notification: String) {
		if notification.as_str() == "login" {
			if settings::is_just_logged_in() {
				settings::clear_just_logged_in();
			} else {
				settings::clear_auth();
			}
		}
	}
}

impl JMComic {
	fn parse_filters<'a>(
		query: Option<&'a str>,
		filters: &'a [FilterValue],
	) -> (&'a str, &'a str, Option<&'a str>) {
		let mut order = "mr";
		let mut category = "";
		let mut keyword = query.filter(|s| !s.is_empty());
		for filter in filters {
			match filter {
				FilterValue::Select { id, value } if !value.is_empty() => match id.as_str() {
					"sort" => order = value,
					"category" if category.is_empty() => category = value,
					"tag" | "genre" => keyword = Some(value),
					_ => {}
				},
				FilterValue::Text { id, value }
					if !value.is_empty() && matches!(id.as_str(), "author" | "title") =>
				{
					keyword = Some(value);
				}
				_ => {}
			}
		}
		(order, category, keyword)
	}

	fn listing_url(id: &str, page: i32) -> Result<String> {
		if let Some(order) = id.strip_prefix("o:") {
			return Ok(net::url::filter(order, "", page));
		}
		if let Some(query) = id.strip_prefix("q:") {
			return Ok(net::url::search(query, "mr", "", page));
		}
		if let Some(slug) = id.strip_prefix("cat:") {
			return Ok(net::url::filter("mr", slug, page));
		}
		bail!("未知列表请求")
	}

	// returns the number of horizontal slices used to scramble the image
	fn calc_scramble_num(ep_id: u64, filename: &str) -> u32 {
		if ep_id < SCRAMBLE_ID {
			return 0;
		}
		if ep_id < 268850 {
			return 10;
		}
		let dot = filename.rfind('.').unwrap_or(filename.len());
		let pic_name = &filename[..dot];
		let hash_hex = format!(
			"{:x}",
			md5::compute(format!("{}{}", ep_id, pic_name).as_bytes())
		);
		let last = hash_hex.chars().last().unwrap_or('0') as u32;
		if ep_id > 421926 {
			(last % 8) * 2 + 2
		} else {
			(last % 10) * 2 + 2
		}
	}
}

fn block_ctx() -> BlockState {
	BlockState::new(settings::blocked_entries())
}

fn search_result(api: &ApiContext, path: &str, block: &BlockState) -> Result<MangaPageResult> {
	let resp: SearchResp = api.get(path)?;
	Ok(MangaPageResult {
		has_next_page: resp.total > PAGE_SIZE,
		entries: resp.into_manga_list(&api.cdn_base, block),
	})
}

fn direct_manga_result(api: &ApiContext, key: &str, block: &BlockState) -> Result<MangaPageResult> {
	let resp: AlbumResp = api.get(&net::url::album(key))?;
	if resp.is_missing() || resp.is_blocked(key, block) {
		return Ok(MangaPageResult::default());
	}
	Ok(MangaPageResult {
		has_next_page: false,
		entries: vec![resp.into_manga(key, &api.cdn_base)],
	})
}

fn visible_album(api: &ApiContext, key: &str, block: &BlockState) -> Result<AlbumResp> {
	let resp: AlbumResp = api.get(&net::url::album(key))?;
	if resp.is_blocked(key, block) {
		bail!("这个内容已经被你屏蔽啦")
	}
	Ok(resp)
}

fn finish_search_result(page: i32, result: MangaPageResult) -> Result<MangaPageResult> {
	if page <= 1 && result.entries.is_empty() {
		bail!("没有找到这样的内容")
	}
	Ok(result)
}

fn parse_manga_key(api: &ApiContext, query: &str) -> Result<Option<String>> {
	let trimmed = query.trim();
	if trimmed.is_empty() {
		return Ok(None);
	}
	if trimmed.chars().all(|ch| ch.is_ascii_digit()) {
		return Ok(Some(trimmed.into()));
	}
	if let Some(key) = extract_id(trimmed, "/album/") {
		return Ok(Some(key));
	}
	if let Some(chapter_key) = extract_id(trimmed, "/photo/") {
		let chapter: ChapterResp = api.get(&net::url::chapter(&chapter_key))?;
		return Ok(Some(chapter.series_key(&chapter_key).into()));
	}
	Ok(None)
}

fn extract_id(url: &str, marker: &str) -> Option<String> {
	let (_, tail) = url.split_once(marker)?;
	let id = tail
		.split(['/', '?', '#'])
		.next()
		.filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()))?;
	Some(id.into())
}

register_source!(
	JMComic,
	Home,
	ListingProvider,
	ImageRequestProvider,
	PageImageProcessor,
	DeepLinkHandler,
	BasicLoginHandler,
	NotificationHandler
);
