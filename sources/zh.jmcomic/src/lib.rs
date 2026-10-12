#![no_std]
use aidoku::{
	BasicLoginHandler, Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, HashMap,
	ImageResponse, Listing, ListingProvider, Manga, MangaPageResult, NotificationHandler, Page,
	PageContent, PageContext, PageImageProcessor, Result, Source,
	alloc::{String, Vec, borrow::Cow, rc::Rc, vec},
	canvas::Rect,
	imports::canvas::{Canvas, ImageRef},
	prelude::*,
};
use core::{cell::RefCell, slice};

mod home;
mod markdown;
mod models;
mod net;
mod novel;
mod settings;

use models::{AlbumResp, BlockState, ChapterResp, ComicItem, SearchResp, manga_list};
use net::ApiContext;

const WEB_URL: &str = "https://18comic.vip";
const NOVEL_PREFIX: &str = "novel/";
const PAGE_SIZE: i32 = 80;
const PROMOTE_PAGE_SIZE: i32 = 27;
const SCRAMBLE_ID: u64 = 220980;
const LOGIN_ATTEMPTS: usize = 3;
// the server cuts search queries at 40 characters
const QUERY_LIMIT: usize = 40;

#[derive(Default)]
struct JMComic {
	api: RefCell<Option<Rc<ApiContext>>>,
}

impl Source for JMComic {
	fn new() -> Self {
		Self::default()
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let api = self.api()?;
		let mut block = settings::block_state();
		let (order, category, keyword, search_type) = Self::parse_filters(query, &filters);

		let Some(keyword) = keyword else {
			return browse(&api, order, category, page, &mut block);
		};
		if block.matches([keyword.as_str()]) {
			return Ok(MangaPageResult::default());
		}
		if let Some(key) = novel::link_key(&api, &keyword)? {
			return novel::direct_result(&api, &key, &block);
		}
		let query: Cow<str> =
			match extract_id(&keyword, "/album/").or_else(|| extract_id(&keyword, "/photo/")) {
				Some(id) => id.into(),
				None if search_type != "site" || is_album_id(&keyword) => keyword.as_str().into(),
				None => {
					let limit = QUERY_LIMIT.saturating_sub(keyword.chars().count() + 1);
					match block.exclusions(limit) {
						Some(exclusions) => format!("{keyword} {exclusions}").into(),
						None => keyword.as_str().into(),
					}
				}
			};
		let path = net::url::search(&query, search_type, order, category, page);
		let with_novels = page == 1 && category.is_empty();
		let (resp, novels): (SearchResp, Option<SearchResp>) = if with_novels {
			let novel_path = net::url::search_novels(&keyword, order);
			let (resp, novels) = api.get_pair(&path, &novel_path)?;
			(resp, Some(novels))
		} else {
			(api.get(&path)?, None)
		};
		if let Some(key) = resp.redirect_aid {
			return direct_manga_result(&api, &key, &block);
		}
		let mut result = resp.into_page_result(page, PAGE_SIZE, &api.cdn_base, &block);
		if let Some(novels) = novels {
			let novels = manga_list(novels.content, &api.cdn_base, &block, ComicItem::into_novel);
			result.entries.splice(0..0, novels);
		}
		Ok(result)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let api = self.api()?;
		if manga.key.starts_with(NOVEL_PREFIX) {
			let mut resp = novel::visible(&api, &manga.key, &settings::block_state())?;
			if needs_chapters {
				manga.chapters = Some(resp.take_chapters());
			}
			if needs_details {
				manga.copy_from(resp.into_manga(&manga.key, &api.cdn_base));
			}
			return Ok(manga);
		}

		let mut resp: AlbumResp = api.get(&net::url::album(&manga.key))?;
		if resp.is_blocked(&manga.key, &settings::block_state()) {
			bail!("此內容已被你封鎖");
		}
		if needs_chapters {
			manga.chapters = Some(resp.take_chapters(&manga.key));
		}
		if needs_details {
			manga.copy_from(resp.into_manga(&manga.key, &api.cdn_base));
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		if settings::block_state().blocks(&manga) {
			bail!("此內容已被你封鎖");
		}
		let api = self.api()?;
		if manga.key.starts_with(NOVEL_PREFIX) {
			return novel::pages(&api, &manga.key, &chapter.key);
		}

		let resp: ChapterResp = api.get(&net::url::chapter(&chapter.key))?;
		if resp.images.is_empty() {
			let album: AlbumResp = api.get(&net::url::album(&manga.key))?;
			return unserved(album.is_unpaid());
		}
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
		let api = self.api()?;
		let mut block = settings::block_state();
		match listing.id.as_str() {
			"novels" => novel::listing(&api, &block),
			"latest" => latest(&api, page, &mut block),
			id => match id.strip_prefix("promo:") {
				Some(group) => home::listing_page(&api, group, page, &mut block),
				None => {
					let (order, category) = Self::listing_filter(id)?;
					browse(&api, order, category, page, &mut block)
				}
			},
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
		if let Some(key) = extract_id(&url, "/photo/") {
			let Some(manga_key) = album_key(&*self.api()?, key)? else {
				return Ok(None);
			};
			let key = key.into();
			return Ok(Some(DeepLinkResult::Chapter { manga_key, key }));
		}
		if let Some(key) = extract_id(&url, "/album/") {
			let key = album_key(&*self.api()?, key)?;
			return Ok(key.map(|key| DeepLinkResult::Manga { key }));
		}
		if let Some(key) = extract_id(&url, "/novelchapters/") {
			let Some(manga_key) = novel::chapter_novel(&*self.api()?, key)? else {
				return Ok(None);
			};
			let key = key.into();
			return Ok(Some(DeepLinkResult::Chapter { manga_key, key }));
		}
		Ok(novel::key(&url).map(|key| DeepLinkResult::Manga { key }))
	}
}

impl BasicLoginHandler for JMComic {
	fn handle_basic_login(&self, _key: String, username: String, password: String) -> Result<bool> {
		for _ in 0..LOGIN_ATTEMPTS {
			let api = self.api()?;
			match api.login(&username, &password) {
				Ok(auth) => {
					settings::set_auth(&auth);
					return Ok(true);
				}
				Err(_) if api.is_usable() => return Ok(false),
				Err(_) => {}
			}
		}
		Ok(false)
	}
}

impl NotificationHandler for JMComic {
	fn handle_notification(&self, notification: String) {
		if notification == "login" && !settings::is_logged_in() {
			settings::clear_auth();
		}
	}
}

impl JMComic {
	fn api(&self) -> Result<Rc<ApiContext>> {
		let mut cached = self.api.borrow_mut();
		if let Some(api) = cached.as_ref().filter(|api| api.is_usable()) {
			return Ok(Rc::clone(api));
		}
		let api = Rc::new(net::context(cached.as_deref())?);
		*cached = Some(Rc::clone(&api));
		Ok(api)
	}

	fn parse_filters(
		query: Option<String>,
		filters: &[FilterValue],
	) -> (&str, &str, Option<String>, &'static str) {
		let mut order = "mr";
		let mut category = "";
		let mut keyword = query;
		let mut search_type = "site";
		for filter in filters {
			let terms: &[String] = match filter {
				FilterValue::Select { id, value } => match id.as_str() {
					"sort" => {
						order = value;
						continue;
					}
					"category" => {
						category = value;
						continue;
					}
					"genre" => slice::from_ref(value),
					_ => continue,
				},
				FilterValue::MultiSelect { id, included, .. } if id == "tag" => included,
				FilterValue::Text { id, value } if id == "author" => slice::from_ref(value),
				_ => continue,
			};
			let by_author = keyword.is_none() && matches!(filter, FilterValue::Text { .. });
			for term in terms {
				let term = term.trim();
				if term.is_empty() {
					continue;
				}
				search_type = if by_author { "author" } else { "site" };
				match &mut keyword {
					Some(keyword) => {
						keyword.push_str(" +");
						keyword.push_str(term);
					}
					None => keyword = Some(term.into()),
				}
			}
		}
		(order, category, keyword, search_type)
	}

	fn listing_filter(id: &str) -> Result<(&str, &str)> {
		if let Some(order) = id.strip_prefix("o:") {
			return Ok((order, ""));
		}
		if let Some(slug) = id.strip_prefix("cat:") {
			return Ok(("mr", slug));
		}
		bail!("未知的列表")
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
		if ep_id >= 421926 {
			(last % 8) * 2 + 2
		} else {
			(last % 10) * 2 + 2
		}
	}
}

fn browse(
	api: &ApiContext,
	order: &str,
	category: &str,
	page: i32,
	block: &mut BlockState,
) -> Result<MangaPageResult> {
	let path = net::url::filter(order, category, page);
	let resp: SearchResp = api.get_list(&path, order, category, block)?;
	Ok(resp.into_page_result(page, PAGE_SIZE, &api.cdn_base, block))
}

fn latest(api: &ApiContext, page: i32, block: &mut BlockState) -> Result<MangaPageResult> {
	let items: Vec<ComicItem> = api.get_list(&net::url::latest(page - 1), "mr", "", block)?;
	Ok(MangaPageResult {
		has_next_page: items.len() == PAGE_SIZE as usize,
		entries: manga_list(items, &api.cdn_base, block, ComicItem::into_manga),
	})
}

fn direct_manga_result(api: &ApiContext, key: &str, block: &BlockState) -> Result<MangaPageResult> {
	let resp: AlbumResp = api.get(&net::url::album(key))?;
	if resp.is_missing() || resp.is_blocked(key, block) {
		return Ok(MangaPageResult::default());
	}
	Ok(MangaPageResult {
		has_next_page: false,
		entries: vec![resp.into_entry(key, &api.cdn_base)],
	})
}

// the server serves paid content without purchase, so only gate when empty
fn unserved(unpaid: bool) -> Result<Vec<Page>> {
	if unpaid {
		bail!("此章節需購買後才能閱讀");
	}
	Ok(Vec::new())
}

fn album_key(api: &ApiContext, id: &str) -> Result<Option<String>> {
	let resp: SearchResp = api.get(&net::url::search(id, "site", "mr", "", 1))?;
	Ok(resp.redirect_aid)
}

fn is_album_id(query: &str) -> bool {
	let query = query.trim();
	let id = match query.split_at_checked(2) {
		Some((prefix, id)) if prefix.eq_ignore_ascii_case("jm") => id.trim_start(),
		_ => query,
	};
	!id.is_empty() && id.bytes().all(|b| b.is_ascii_digit())
}

fn extract_id<'a>(url: &'a str, marker: &str) -> Option<&'a str> {
	let (_, tail) = url.split_once(marker)?;
	tail.split(['/', '?', '#'])
		.next()
		.filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()))
}

register_source!(
	JMComic,
	Home,
	ListingProvider,
	PageImageProcessor,
	DeepLinkHandler,
	BasicLoginHandler,
	NotificationHandler
);
