#![no_std]

use aidoku::{
	BasicLoginHandler, Chapter, DeepLinkHandler, DeepLinkResult, DynamicFilters, DynamicSettings,
	Filter, FilterValue, GroupSetting, Home, HomeLayout, ImageRequestProvider, Listing,
	ListingProvider, Manga, MangaPageResult, NotificationHandler, Page, PageContent, PageContext,
	Result, SelectFilter, Setting, Source,
	alloc::{String, Vec, borrow::Cow, format, string::ToString},
	helpers::uri::QueryParameters,
	imports::net::Request,
	prelude::*,
};

mod helpers;
mod home;
mod models;
mod net;
mod settings;

pub const BASE_URL: &str = "https://www.zaimanhua.com";
pub const V4_API_URL: &str = "https://v4api.zaimanhua.com/app/v1";
pub const ACCOUNT_API: &str = "https://account-api.zaimanhua.com/v1";
pub const SIGN_API: &str = "https://i.zaimanhua.com/lpi/v1";
pub const NEWS_URL: &str = "https://news.zaimanhua.com";
pub const WEB_URL: &str = "https://manhua.zaimanhua.com";
pub const USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 10) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36";

struct Zaimanhua;

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
		for filter in &filters {
			if let FilterValue::Text { id, value } = filter {
				if id == "author" {
					return helpers::search_by_author(value, page);
				}
				return helpers::search_by_keyword(value, page);
			}
		}

		if let Some(keyword) = query.as_deref()
			&& !keyword.is_empty()
		{
			return helpers::search_by_keyword(keyword, page);
		}

		let mut sort_type: Option<&str> = None;
		let mut zone: Option<&str> = None;
		let mut status: Option<&str> = None;
		let mut cate: Option<&str> = None;
		let mut theme: Option<&str> = None;
		let mut rank_mode: Option<&str> = None;
		let mut genre: Option<&str> = None;

		for filter in &filters {
			if let FilterValue::Select { id, value } = filter {
				match id.as_str() {
					"排序" => sort_type = Some(value.as_str()),
					"地区" => zone = Some(value.as_str()),
					"状态" => status = Some(value.as_str()),
					"受众" => cate = Some(value.as_str()),
					"题材" => theme = Some(value.as_str()),
					"榜单" => rank_mode = Some(value.as_str()),
					"genre" => genre = Some(value.as_str()),
					_ => {}
				}
			}
		}

		if let Some(mode @ ("1" | "2" | "3" | "4")) = rank_mode {
			let by_time = match mode {
				"2" => 1,
				"3" => 2,
				"4" => 3,
				_ => 0,
			};
			let url = net::urls::rank(by_time, page);
			let response: models::ApiResponse<Vec<models::RankItem>> =
				net::auth_request(&url, settings::get_token().as_deref())?.json_owned()?;
			let data = response.data.unwrap_or_default();
			return Ok(models::manga_list_from_ranks(data));
		}

		let genre = genre.map(helpers::resolve_theme_id).transpose()?;

		let mut qs = QueryParameters::new();
		qs.push("sortType", Some(sort_type.unwrap_or("1")));
		qs.push("cate", Some(cate.unwrap_or("0")));
		qs.push("status", Some(status.unwrap_or("0")));
		qs.push("zone", Some(zone.unwrap_or("0")));
		qs.push("theme", Some(genre.as_deref().or(theme).unwrap_or("0")));

		let url = format!("{}&size=20", net::urls::filter(&qs.to_string(), page));
		let response: models::ApiResponse<models::FilterData> =
			net::auth_request(&url, settings::get_token().as_deref())?.json_owned()?;
		let data = response
			.data
			.map(|d| d.comic_list)
			.ok_or_else(|| error!("筛选数据缺失"))?;
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
			net::auth_request(&url, settings::get_token().as_deref())?.json_owned()?;

		if response.errno.unwrap_or(0) != 0 {
			let errmsg = response.errmsg.as_deref().unwrap_or("未知错误");
			bail!("{errmsg}");
		}

		let detail_data = response.data.ok_or_else(|| error!("接口数据缺失"))?;
		let manga_detail = detail_data.data.ok_or_else(|| error!("详情数据缺失"))?;

		if needs_chapters {
			manga.chapters = Some(manga_detail.to_chapters(&manga.key));
		}
		if needs_details {
			manga.copy_from(manga_detail.into_manga(manga.key.clone()));
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let (comic_id, chapter_id) = chapter
			.key
			.split_once('/')
			.unwrap_or((manga.key.as_str(), chapter.key.as_str()));

		let url = net::urls::chapter(comic_id, chapter_id);
		let response: models::ApiResponse<models::ChapterData> =
			net::auth_request(&url, settings::get_token().as_deref())?.json_owned()?;
		let chapter_data = response.data.ok_or_else(|| error!("章节数据缺失"))?;
		let page_data = chapter_data.data;

		let page_urls = page_data
			.page_url_hd
			.or(page_data.page_url)
			.ok_or_else(|| error!("页面地址缺失"))?;

		let pages = page_urls
			.into_iter()
			.map(|url| Page {
				content: PageContent::url(url),
				..Default::default()
			})
			.collect();

		Ok(pages)
	}
}

impl ImageRequestProvider for Zaimanhua {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		let resolved = net::resolve_url(&url);
		Ok(Request::get(resolved)?
			.header("User-Agent", USER_AGENT)
			.header("Referer", &format!("{BASE_URL}/")))
	}
}

impl DeepLinkHandler for Zaimanhua {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		if (url.contains("/manga/") || url.contains("/comic/")) && !url.contains("chapter") {
			let id = if let Some(pos) = url.find("id=") {
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

		if let Some(start) = url.find("/chapter/") {
			let mut segments = url
				.get(start + 9..)
				.unwrap_or("")
				.split('/')
				.filter(|s| !s.is_empty());

			if let (Some(comic_id), Some(chapter_id)) = (segments.next(), segments.next())
				&& comic_id.chars().all(|c| c.is_ascii_digit())
				&& chapter_id.chars().all(|c| c.is_ascii_digit())
			{
				return Ok(Some(DeepLinkResult::Chapter {
					manga_key: comic_id.into(),
					key: format!("{comic_id}/{chapter_id}"),
				}));
			}
		}

		Ok(None)
	}
}

impl BasicLoginHandler for Zaimanhua {
	fn handle_basic_login(&self, key: String, username: String, password: String) -> Result<bool> {
		if key != "login" {
			bail!("登录入口无效");
		}
		if username.is_empty() || password.is_empty() {
			return Ok(false);
		}

		match net::login(&username, &password) {
			Ok(Some(token)) => {
				settings::set_token(&token);
				settings::set_credentials(&username, &password);
				settings::set_just_logged_in();
				net::perform_silent_updates();
				Ok(true)
			}
			_ => Ok(false),
		}
	}
}

impl NotificationHandler for Zaimanhua {
	fn handle_notification(&self, notification: String) {
		if notification.as_str() == "login" {
			// Flag-based logout detection
			if settings::is_just_logged_in() {
				settings::clear_just_logged_in();
			} else {
				settings::clear_token();
				settings::clear_checkin_flag();
				settings::clear_user_cache();
				settings::reset_dependent_settings();
			}
		}
	}
}

impl DynamicSettings for Zaimanhua {
	fn get_dynamic_settings(&self) -> Result<Vec<Setting>> {
		let mut settings: Vec<Setting> = Vec::new();

		if settings::get_token().is_some() {
			let username = settings::get_credentials()
				.map(|(u, _)| u)
				.unwrap_or_else(|| "未知用户".into());

			let user_cache = settings::get_user_cache();

			let (level_str, status_str) = if let Some(ref cache) = user_cache {
				let checkin_status = if cache.is_sign {
					"已签到"
				} else {
					"未签到"
				};
				(format!("Lv.{}", cache.level), checkin_status.to_string())
			} else {
				("Lv.?".to_string(), "获取中...".to_string())
			};

			let mut footer_text = format!(
				"用户：{username} | 等级：{level_str} | {status_str}\n※ 访问内容受等级与时段限制"
			);

			if user_cache.is_none_or(|c| c.level < 1) {
				footer_text = format!("{footer_text}\n※ 绑定手机号码访问更多内容");
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
		if listing.id == "rank-monthly" {
			let url = net::urls::rank(2, page);
			let response: models::ApiResponse<Vec<models::RankItem>> =
				net::auth_request(&url, settings::get_token().as_deref())?.json_owned()?;
			let data = response.data.unwrap_or_default();
			return Ok(models::manga_list_from_ranks(data));
		}

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
				let token = settings::get_token().ok_or_else(|| error!("请先登录"))?;

				let url = net::urls::sub_list(page);
				let response: models::ApiResponse<models::SubscribeData> =
					net::auth_request(&url, Some(&token))?.json_owned()?;
				let data = response
					.data
					.map(|d| d.sub_list)
					.ok_or_else(|| error!("订阅数据缺失"))?;
				return Ok(models::manga_list_from_subscribes(data));
			}
			_ => bail!("未知列表请求"),
		};

		let url = format!("{}&size=20", net::urls::filter(filter_param, page));
		let response: models::ApiResponse<models::FilterData> =
			net::auth_request(&url, settings::get_token().as_deref())?.json_owned()?;
		let data = response
			.data
			.map(|d| d.comic_list)
			.ok_or_else(|| error!("筛选数据缺失"))?;
		Ok(models::manga_list_from_filter(data))
	}
}

impl DynamicFilters for Zaimanhua {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		let url = net::urls::classify();
		let response: models::ApiResponse<models::ClassifyData> =
			net::get_request(&url)?.json_owned()?;
		let data = response.data.ok_or_else(|| error!("分类数据缺失"))?;

		let mut filters = Vec::with_capacity(data.classify_list.len());

		for group in data.classify_list {
			let (filter_id, is_genre) = match group.id {
				1 => ("题材", true),
				6 => ("受众", false),
				5 => ("状态", false),
				4 => ("地区", false),
				_ => continue,
			};

			let mut options = Vec::with_capacity(group.list.len());
			let mut ids = Vec::with_capacity(group.list.len());

			for tag in group.list {
				options.push(Cow::Owned(models::normalize_tag_name(tag.tag_name)));
				ids.push(Cow::Owned(tag.tag_id.to_string()));
			}

			filters.push(
				SelectFilter {
					id: Cow::Borrowed(filter_id),
					title: Some(Cow::Borrowed(filter_id)),
					is_genre,
					options,
					ids: Some(ids),
					..Default::default()
				}
				.into(),
			);
		}

		Ok(filters)
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
	DynamicSettings,
	DynamicFilters
);
