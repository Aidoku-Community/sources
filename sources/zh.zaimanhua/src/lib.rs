#![no_std]

use aidoku::{
	BasicLoginHandler, Chapter, DeepLinkHandler, DeepLinkResult, DynamicSettings, FilterValue,
	GroupSetting, Home, HomeLayout, ImageRequestProvider, Listing, ListingProvider, LoginMethod,
	LoginSetting, Manga, MangaPageResult, NotificationHandler, Page, PageContent, PageContext,
	Result, Setting, Source, ToggleSetting,
	alloc::{String, Vec, format, vec},
	imports::net::Request,
	prelude::*,
};

mod helpers;
mod home;
mod models;
mod net;
mod settings;

use helpers::{V4_API_URL, get_api_request};

pub const BASE_URL: &str = "https://www.zaimanhua.com/";

struct Zaimanhua;

impl Source for Zaimanhua {
	fn new() -> Self {
		// Try auto check-in on source init if logged in and not checked in today
		if let Some(token) = settings::get_token()
			&& settings::get_auto_checkin()
			&& !settings::has_checkin_flag()
			&& let Ok(true) = net::check_in(&token)
		{
			settings::set_last_checkin("done");
		}
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		for filter in filters.iter() {
			if let FilterValue::Text { id, value } = filter {
				if id == "author" {
					return helpers::search_by_author(value, page);
				}
				return helpers::search_by_keyword(value, page);
			}
		}

		if let Some(ref keyword) = query
			&& !keyword.is_empty()
		{
			return helpers::search_by_keyword(keyword, page);
		}

		helpers::browse_with_filters(&filters, page)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{}/comic/detail/{}?channel=android", V4_API_URL, manga.key);

		let response: models::ApiResponse<models::DetailData> =
			get_api_request(&url)?.json_owned()?;

		if response.errno.unwrap_or(0) != 0 {
			let errmsg = response.errmsg.as_deref().unwrap_or("Unknown error");
			return Err(error!("{}", errmsg));
		}

		let detail_data = response.data.ok_or_else(|| error!("Missing data"))?;
		let manga_detail = detail_data
			.data
			.ok_or_else(|| error!("Missing nested data (API error?)"))?;

		if needs_details {
			let details = manga_detail.clone().into_manga(manga.key.clone());
			manga.title = details.title;
			manga.cover = details.cover;
			manga.description = details.description;
			manga.authors = details.authors;
			manga.tags = details.tags;
			manga.status = details.status;
			manga.content_rating = details.content_rating;
			manga.viewer = details.viewer;
			manga.url = details.url;
		}

		if needs_chapters {
			manga.chapters = Some(manga_detail.into_chapters(&manga.key));
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let parts: Vec<&str> = chapter.key.split('/').collect();
		let (comic_id, chapter_id) = if parts.len() == 2 {
			(parts[0], parts[1])
		} else {
			(manga.key.as_str(), chapter.key.as_str())
		};

		let url = format!("{}/comic/chapter/{}/{}", V4_API_URL, comic_id, chapter_id);

		let response: models::ApiResponse<models::ChapterData> =
			get_api_request(&url)?.json_owned()?;
		let chapter_data = response.data.ok_or_else(|| error!("Missing data"))?;
		let page_data = chapter_data.data;

		let page_urls = page_data
			.page_url_hd
			.or(page_data.page_url)
			.ok_or_else(|| error!("Missing page_url"))?;

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

impl ImageRequestProvider for Zaimanhua {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?
			.header("User-Agent", net::USER_AGENT)
			.header("Referer", BASE_URL))
	}
}

impl DeepLinkHandler for Zaimanhua {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		// Case 1: Manga details (e.g., /comic/12345 or id=12345)
		if (url.contains("/manga/") || url.contains("/comic/")) && !url.contains("chapter") {
			let id = if let Some(pos) = url.find("id=") {
				url[pos + 3..].split('&').next().unwrap_or("")
			} else {
				url.split('/').rfind(|s| !s.is_empty()).unwrap_or("")
			};

			if !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()) {
				return Ok(Some(DeepLinkResult::Manga { key: id.into() }));
			}
		}

		// Case 2: Chapter pages (e.g., /comic/chapter/12345/67890)
		if url.contains("/chapter/") {
			// Extract comic_id and chapter_id
			// URL format: .../chapter/{comic_id}/{chapter_id}
			if let Some(start) = url.find("/chapter/") {
				let parts: Vec<&str> = url[start + 9..]
					.split('/')
					.filter(|s| !s.is_empty())
					.collect();
				if parts.len() >= 2 {
					let comic_id = parts[0];
					let chapter_id = parts[1];

					if comic_id.chars().all(|c| c.is_ascii_digit())
						&& chapter_id.chars().all(|c| c.is_ascii_digit())
					{
						return Ok(Some(DeepLinkResult::Chapter {
							manga_key: comic_id.into(),
							key: format!("{}/{}", comic_id, chapter_id),
						}));
					}
				}
			}
		}

		Ok(None)
	}
}

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
				settings::set_just_logged_in(); // Mark for logout detection
				// Auto check-in after login (if enabled and not already done)
				if settings::get_auto_checkin()
					&& !settings::has_checkin_flag()
					&& let Ok(true) = net::check_in(&token)
				{
				settings::set_last_checkin("done");
				}
				Ok(true)
			}
			_ => Ok(false),
		}
	}
}

impl NotificationHandler for Zaimanhua {
	fn handle_notification(&self, notification: String) {
		match notification.as_str() {
			"checkin" => {
				if let Some(token) = settings::get_token() {
					let _ = net::check_in(&token);
				}
			}
			"login" => {
				// Flag-based logout detection
				if settings::is_just_logged_in() {
					// Just logged in - clear flag, don't logout
					settings::clear_just_logged_in();
				} else {
					// Not just logged in = user logged out
					settings::clear_token();
					settings::clear_checkin_flag();
					settings::clear_hidden_cache();
				}
			}
			"clearHiddenCache" => {
				settings::clear_hidden_cache();
			}
			_ => {}
		}
	}
}

// === Dynamic Settings for User Info Display ===

impl DynamicSettings for Zaimanhua {
	fn get_dynamic_settings(&self) -> Result<Vec<Setting>> {
		let mut settings_list: Vec<Setting> = Vec::new();

		let is_logged_in = settings::get_token().is_some();

		// Try to get user info if logged in
		let user_info_opt = if is_logged_in {
			if let Some(token) = settings::get_token() {
				net::get_user_info(&token)
					.ok()
					.and_then(|info_data| info_data.user_info)
			} else {
				None
			}
		} else {
			None
		};

		// Prepare checkin subtitle
		let checkin_subtitle = user_info_opt.as_ref().map(|info| {
			let is_signed = info.is_sign.unwrap_or(false);
			if is_signed {
				"今日已签到"
			} else {
				"今日未签到"
			}
		});

		let mut account_items: Vec<Setting> = Vec::new();

		// Login (with logout notification)
		account_items.push(
			LoginSetting {
				key: "login".into(),
				title: "登录".into(),
				notification: Some("login".into()), // Fires on login state change (GigaViewer pattern)
				method: LoginMethod::Basic,
				refreshes: Some(vec!["settings".into(), "content".into(), "listings".into()]),
				..Default::default()
			}
			.into(),
		);

		// Auto check-in (always show, but with status subtitle only when logged in)
		account_items.push(
			ToggleSetting {
				key: "autoCheckin".into(),
				title: "自动签到".into(),
				subtitle: checkin_subtitle.map(|s| s.into()),
				default: false,
				..Default::default()
			}
			.into(),
		);

		// Enhanced mode (only show when logged in)
		if is_logged_in {
			account_items.push(
				ToggleSetting {
					key: "enhancedMode".into(),
					title: "增强浏览".into(),
					subtitle: Some("获取更多内容".into()),
					default: false,
					refreshes: Some(vec!["content".into(), "listings".into(), "settings".into()]),
					..Default::default()
				}
				.into(),
			);

			// Hidden content toggle (only show when Enhanced Mode is enabled)
			if settings::get_enhanced_mode() {
				account_items.push(
					ToggleSetting {
						key: "showHiddenContent".into(),
						title: "隐藏内容".into(),
						subtitle: Some("搜索和浏览时包含隐藏漫画".into()),
						default: false,
						refreshes: Some(vec!["content".into(), "listings".into()]),
						notification: Some("clearHiddenCache".into()),
						..Default::default()
					}
					.into(),
				);
			}
		}

		settings_list.push(
			GroupSetting {
				key: "account".into(),
				title: "账号".into(),
				items: account_items,
				..Default::default()
			}
			.into(),
		);

		if let Some(user_info) = user_info_opt {
			// Extract info
			let username = user_info
				.username
				.as_deref()
				.or(user_info.nickname.as_deref())
				.unwrap_or("未知用户");

			let level = user_info.level.unwrap_or(0);

			// Build info footer
			let footer_text = format!("用户：{} | 等级：Lv.{}", username, level);

			// Add user info group
			settings_list.push(
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

		Ok(settings_list)
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
			let url = format!(
				"{}/comic/rank/list?rank_type=0&by_time=2&page={}&size=20",
				V4_API_URL, page
			);
			let response: models::ApiResponse<Vec<models::RankItem>> =
				get_api_request(&url)?.json_owned()?;
			let data = response
				.data
				.ok_or_else(|| aidoku::error!("No data in rank response"))?;
			return Ok(models::manga_list_from_ranks(data));
		}

		// Handle filter-based listings
		let url = match listing.id.as_str() {
			"latest" => format!(
				"{}/comic/filter/list?sortType=1&page={}&size=20",
				V4_API_URL, page
			),
			"ongoing" => format!(
				"{}/comic/filter/list?status=2309&page={}&size=20",
				V4_API_URL, page
			),
			"completed" => format!(
				"{}/comic/filter/list?status=2310&page={}&size=20",
				V4_API_URL, page
			),
			"short" => format!(
				"{}/comic/filter/list?status=29205&page={}&size=20",
				V4_API_URL, page
			),
			"shounen" => format!(
				"{}/comic/filter/list?cate=3262&page={}&size=20",
				V4_API_URL, page
			),
			"shoujo" => format!(
				"{}/comic/filter/list?cate=3263&page={}&size=20",
				V4_API_URL, page
			),
			"seinen" => format!(
				"{}/comic/filter/list?cate=3264&page={}&size=20",
				V4_API_URL, page
			),
			"josei" => format!(
				"{}/comic/filter/list?cate=13626&page={}&size=20",
				V4_API_URL, page
			),
			"subscribe" => {
				let token = settings::get_token()
					.ok_or_else(|| aidoku::error!("请先登录以查看订阅列表"))?;

				let url = format!(
					"{}/comic/sub/list?status=0&firstLetter=&page={}&size=50",
					V4_API_URL, page
				);

				let response: models::ApiResponse<models::SubscribeData> =
					net::auth_request(&url, &token)?.json_owned()?;
				let data = response
					.data
					.ok_or_else(|| aidoku::error!("Invalid subscribe response"))?;

				return Ok(models::manga_list_from_subscribes(data.sub_list));
			}
			_ => return Err(aidoku::error!("Unknown listing: {}", listing.id)),
		};

		let response: models::ApiResponse<models::FilterData> =
			get_api_request(&url)?.json_owned()?;
		let data = response
			.data
			.ok_or_else(|| aidoku::error!("No data in filter response"))?;
		Ok(models::manga_list_from_filter(data.comic_list))
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
