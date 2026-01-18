#![no_std]
extern crate alloc;

use aidoku::{
	BasicLoginHandler, Chapter, DeepLinkHandler, DeepLinkResult, DynamicSettings, FilterValue,
	GroupSetting, Home, HomeLayout, ImageRequestProvider, Listing, ListingProvider,
	Manga, MangaPageResult, NotificationHandler, Page, PageContent, PageContext,
	Result, Setting, Source,
	alloc::{String, Vec, format, string::ToString},
	helpers::uri::QueryParameters,
	imports::net::Request,
	prelude::*,
};

mod helpers;
mod home;
mod models;
mod net;
mod settings;

pub const BASE_URL: &str = "https://www.zaimanhua.com/";
pub const V4_API_URL: &str = "https://v4api.zaimanhua.com/app/v1";
pub const ACCOUNT_API: &str = "https://account-api.zaimanhua.com/v1/";
pub const SIGN_API: &str = "https://i.zaimanhua.com/lpi/v1/";
pub const NEWS_URL: &str = "https://news.zaimanhua.com";
pub const WEB_URL: &str = "https://manhua.zaimanhua.com";
pub const USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 10) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36";

struct Zaimanhua;

// === Main Source Implementation ===
// Core logic for manga listing, updates, and page fetching.

impl Source for Zaimanhua {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		// Handle text filters (author search or keyword search)
		for filter in filters.iter() {
			if let FilterValue::Text { id, value } = filter {
				if id == "author" {
					return helpers::search_by_author(value, page);
				}
				return helpers::search_by_keyword(value, page);
			}
		}

		// Handle query bar search
		if let Some(ref keyword) = query
			&& !keyword.is_empty()
		{
			return helpers::search_by_keyword(keyword, page);
		}

		// === Filter Browsing (inline for Locality of Behavior) ===
		let mut sort_type: Option<&str> = None;
		let mut zone: Option<&str> = None;
		let mut status: Option<&str> = None;
		let mut cate: Option<&str> = None;
		let mut theme: Option<&str> = None;
		let mut rank_mode: Option<&str> = None;

		for filter in filters.iter() {
			if let FilterValue::Select { id, value } = filter {
				match id.as_str() {
					"排序" => sort_type = Some(value.as_str()),
					"地区" => zone = Some(value.as_str()),
					"状态" => status = Some(value.as_str()),
					"受众" => cate = Some(value.as_str()),
					"题材" => theme = Some(value.as_str()),
					"榜单" => rank_mode = Some(value.as_str()),
					_ => {}
				}
			}
		}

		// Handle rank mode
		if let Some(mode @ ("1" | "2" | "3" | "4")) = rank_mode {
			let by_time = mode.parse::<i32>().unwrap_or(1) - 1;
			let url = net::urls::rank(by_time, page);
			let response: models::ApiResponse<Vec<models::RankItem>> =
				net::auth_request(&url, settings::get_current_token().as_deref())?.json_owned()?;
			let data: Vec<models::RankItem> = response.data.unwrap_or_default();
			if data.is_empty() {
				return Ok(MangaPageResult { entries: Vec::new(), has_next_page: false });
			}
			return Ok(models::manga_list_from_ranks(data));
		}

		// Build filter query
		let mut qs = QueryParameters::new();
		qs.push("sortType", Some(sort_type.unwrap_or("1")));
		qs.push("cate", Some(cate.unwrap_or("0")));
		qs.push("status", Some(status.unwrap_or("0")));
		qs.push("zone", Some(zone.unwrap_or("0")));
		qs.push("theme", Some(theme.unwrap_or("0")));

		let url = format!("{}&size=20", net::urls::filter(&qs.to_string(), page));
		let response: models::ApiResponse<models::FilterData> =
			net::auth_request(&url, settings::get_current_token().as_deref())?.json_owned()?;
		let data = response.data
			.map(|d| d.comic_list)
			.ok_or_else(|| error!("Missing filter data"))?;
		Ok(models::manga_list_from_filter(data))
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = net::urls::detail(manga.key.parse::<i64>().unwrap_or(0));
		let response: models::ApiResponse<models::DetailData> =
			net::auth_request(&url, settings::get_current_token().as_deref())?.json_owned()?;

		if response.errno.unwrap_or(0) != 0 {
			let errmsg = response.errmsg.as_deref().unwrap_or("未知错误");
			return Err(error!("{}", errmsg));
		}

		let detail_data = response.data.ok_or_else(|| error!("Missing API data"))?;
		let manga_detail = detail_data
			.data
			.ok_or_else(|| error!("Missing nested data"))?;

		match (needs_details, needs_chapters) {
			(true, true) => {
				// Clone for chapters, consume for details
				let chapters_source = manga_detail.clone();
				manga.copy_from(manga_detail.into_manga(manga.key.clone()));
				manga.chapters = Some(chapters_source.into_chapters(&manga.key));
			}
			(true, false) => {
				manga.copy_from(manga_detail.into_manga(manga.key.clone()));
			}
			(false, true) => {
				manga.chapters = Some(manga_detail.into_chapters(&manga.key));
			}
			(false, false) => {}
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let (comic_id, chapter_id) = chapter.key.split_once('/')
			.unwrap_or((manga.key.as_str(), chapter.key.as_str()));

		let url = net::urls::chapter(comic_id, chapter_id);
		let response: models::ApiResponse<models::ChapterData> =
			net::auth_request(&url, settings::get_current_token().as_deref())?.json_owned()?;
		let chapter_data = response.data.ok_or_else(|| error!("Missing chapter data"))?;
		let page_data = chapter_data.data;

		let page_urls = page_data
			.page_url_hd
			.or(page_data.page_url)
			.ok_or_else(|| error!("Missing page URLs"))?;

		let pages = page_urls
			.into_iter()
			.map(|url| Page {
				content: PageContent::url(&url),
				..Default::default()
			})
			.collect();

		Ok(pages)
	}
}

// === Image Request Provider ===
// Custom referer handling for image protection.
impl ImageRequestProvider for Zaimanhua {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		let resolved = net::resolve_url(&url);
		Ok(Request::get(resolved)?
			.header("User-Agent", USER_AGENT)
			.header("Referer", BASE_URL))
	}
}

// === Deep Link Handler ===
// Parse partial URLs for manga/chapter redirection.
impl DeepLinkHandler for Zaimanhua {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		// Handle manga details URL (compatibility for various formats)
		if (url.contains("/manga/") || url.contains("/comic/")) && !url.contains("chapter") {
			let id = if let Some(pos) = url.find("id=") {
				// Safe substring access
				url.get(pos + 3..)
					.and_then(|s| s.split('&').next())
					.unwrap_or("")
			} else {
				url.split('/').rfind(|s| !s.is_empty()).unwrap_or("")
			};

			if !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()) {
				return Ok(Some(DeepLinkResult::Manga { key: id.into() }));
			}
		}

		// Handle chapter pages URL
		if let Some(start) = url.find("/chapter/") {
			// Safe substring access using iterator
			let mut segments = url.get(start + 9..)
				.unwrap_or("")
				.split('/')
				.filter(|s| !s.is_empty());

			if let (Some(comic_id), Some(chapter_id)) = (segments.next(), segments.next())
				&& comic_id.chars().all(|c| c.is_ascii_digit())
				&& chapter_id.chars().all(|c| c.is_ascii_digit())
			{
				return Ok(Some(DeepLinkResult::Chapter {
					manga_key: comic_id.into(),
					key: format!("{}/{}", comic_id, chapter_id),
				}));
			}
		}

		Ok(None)
	}
}

// === Login & Auth Handler ===
// Basic username/password login flow.
impl BasicLoginHandler for Zaimanhua {
	fn handle_basic_login(&self, key: String, username: String, password: String) -> Result<bool> {
		if key != "login" {
			bail!("Invalid login key: `{key}`");
		}

		if password.is_empty() {
			return Ok(false);
		}

		match net::login(&username, &password) {
			Ok(Some(token)) => {
				settings::set_token(&token);
				settings::set_credentials(&username, &password);
				settings::set_just_logged_in();

				// Update user profile immediately upon login
				let _ = net::refresh_user_profile(&token);

				if settings::get_auto_checkin()
					&& !settings::has_checkin_flag()
					&& let Ok(true) = net::check_in(&token)
				{
					settings::set_last_checkin();
					// Force refresh again if check-in succeeded (to update points/status)
					let _ = net::refresh_user_profile(&token);
				}
				Ok(true)
			}
			_ => Ok(false),
		}
	}
}

// === Notification Logic ===
// Handle async events like login state changes.
impl NotificationHandler for Zaimanhua {
	fn handle_notification(&self, notification: String) {
		if notification.as_str() == "login" {
			// Flag-based logout detection
			if settings::is_just_logged_in() {
				// Just logged in - clear flag, don't logout
				settings::clear_just_logged_in();
			} else {
				// Not just logged in = user logged out
				settings::clear_token();
				settings::clear_checkin_flag();
				settings::clear_user_cache();
				settings::reset_dependent_settings();
			}
		}
	}
}

// === Dynamic Settings ===
// Dynamic content (user info with checkin status) is returned here.
// Static settings are defined in res/settings.json.
impl DynamicSettings for Zaimanhua {
	fn get_dynamic_settings(&self) -> Result<Vec<Setting>> {
		let mut settings: Vec<Setting> = Vec::new();

		// User Info Display (with Fallback)
		if settings::get_token().is_some() {
			let (username, _) = settings::get_credentials().unwrap_or(("未知用户".into(), "".into()));

			let (level_str, status_str, level_warning) = if let Some(user_cache) = settings::get_user_cache() {
				let checkin_status = if user_cache.is_sign { "已签到" } else { "未签到" };
				let warning = if user_cache.level < 1 {
					Some("※ 增强浏览需要等级达到 Lv.1 可用 (绑定手机号码)")
				} else {
					None
				};
				(format!("Lv.{}", user_cache.level), checkin_status.to_string(), warning)
			} else {
				// Fallback for missing cache
				("Lv.?".to_string(), "获取中...".to_string(), None)
			};

			let mut footer_text = format!("用户：{} | 等级：{} | {}", username, level_str, status_str);
			
			// Enhanced mode active: show general note first
			if settings::get_enhanced_mode() {
				footer_text = format!("{}\n※ 访问内容受等级与时段限制", footer_text);
			}
			
			// Level warning (when < Lv.1)
			if let Some(warning) = level_warning {
				footer_text = format!("{}\n{}", footer_text, warning);
			}

			settings.push(
				GroupSetting {
					key: "userInfo".into(),
					title: "账号信息".into(),
					items: Vec::new(),
					footer: Some(footer_text.into()),
					..Default::default()
				}
				.into(),
			);
		}

		Ok(settings)
	}
}

impl Home for Zaimanhua {
	fn get_home(&self) -> Result<HomeLayout> {
		home::get_home_layout()
	}
}

impl ListingProvider for Zaimanhua {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		// Handle rank listings (use rank API)
		if listing.id == "rank-monthly" {
			let url = net::urls::rank(2, page);
			let response: models::ApiResponse<Vec<models::RankItem>> =
				net::auth_request(&url, settings::get_current_token().as_deref())?.json_owned()?;
			let data = response.data.unwrap_or_default();
			if data.is_empty() {
				return Ok(MangaPageResult { entries: Vec::new(), has_next_page: false });
			}
			return Ok(models::manga_list_from_ranks(data));
		}

		// Handle filter-based listings
		let filter_param = match listing.id.as_str() {
			"latest" => "sortType=1",
			"ongoing" => "status=2309",
			"completed" => "status=2310",
			"short" => "status=29205",
			"shounen" => "cate=3262",
			"shoujo" => "cate=3263",
			"seinen" => "cate=3264",
			"josei" => "cate=13626",
			"subscribe" => {
				let token = settings::get_token()
					.ok_or_else(|| aidoku::error!("请先登录以查看订阅列表"))?;

				let url = net::urls::sub_list(page);
				let response: models::ApiResponse<models::SubscribeData> =
					net::auth_request(&url, Some(&token))?.json_owned()?;
				let data = response
					.data
					.map(|d| d.sub_list)
					.ok_or_else(|| aidoku::error!("Missing subscribe data"))?;
				return Ok(models::manga_list_from_subscribes(data));
			}
			_ => return Err(aidoku::error!("Unknown listing: {}", listing.id)),
		};

		let url = format!("{}&size=20", net::urls::filter(filter_param, page));
		let response: models::ApiResponse<models::FilterData> =
			net::auth_request(&url, settings::get_current_token().as_deref())?.json_owned()?;
		let data = response
			.data
			.map(|d| d.comic_list)
			.ok_or_else(|| aidoku::error!("Missing filter data"))?;
		Ok(models::manga_list_from_filter(data))
	}
}

register_source!(
	Zaimanhua,
	Home,
	ListingProvider,
	ImageRequestProvider,
	DeepLinkHandler,
	BasicLoginHandler,
	NotificationHandler,
	DynamicSettings
);
