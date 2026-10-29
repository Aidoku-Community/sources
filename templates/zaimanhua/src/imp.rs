use crate::{Params, helpers, net, settings};
use aidoku::{
	Chapter, DeepLinkResult, Filter, FilterValue, GroupSetting, HomeLayout, Listing, Manga,
	MangaPageResult, Page, Result, Setting,
	alloc::{String, Vec, vec},
	prelude::*,
};

pub trait Impl {
	fn new() -> Self;

	fn params(&self) -> Params;

	fn get_search_manga_list(
		&self,
		params: &Params,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult>;

	fn get_manga_update(
		&self,
		params: &Params,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga>;

	fn get_page_list(&self, params: &Params, manga: Manga, chapter: Chapter) -> Result<Vec<Page>>;

	fn get_manga_list(
		&self,
		params: &Params,
		listing: Listing,
		page: i32,
	) -> Result<MangaPageResult>;

	fn get_home(&self, params: &Params) -> Result<HomeLayout>;

	fn get_dynamic_filters(&self, params: &Params) -> Result<Vec<Filter>>;

	fn handle_deep_link(&self, params: &Params, url: &str) -> Result<Option<DeepLinkResult>>;

	fn search(&self, params: &Params, keyword: &str, page: i32) -> Result<MangaPageResult> {
		let link = if page == 1 {
			self.handle_deep_link(params, keyword)?
		} else {
			None
		};
		match link {
			Some(DeepLinkResult::Manga { key }) => helpers::search_key(params.kind, &key),
			Some(DeepLinkResult::Chapter { manga_key, .. }) => {
				helpers::search_key(params.kind, &manga_key)
			}
			_ => helpers::search(params.kind, keyword, page),
		}
	}

	fn handle_basic_login(&self, _key: String, username: String, password: String) -> Result<bool> {
		let Some(token) = net::login(&username, &password)? else {
			return Ok(false);
		};
		settings::set_token(&token);
		net::sync_account();
		Ok(true)
	}

	fn handle_notification(&self, notification: String) {
		if notification == "login" && !settings::is_logged_in() {
			settings::logout();
		}
	}

	fn get_dynamic_settings(&self) -> Result<Vec<Setting>> {
		let Some(profile) = settings::get_profile() else {
			return Ok(Vec::new());
		};
		let sign = if profile.is_sign {
			"已签到"
		} else {
			"未签到"
		};
		Ok(vec![
			GroupSetting {
				title: "账号信息".into(),
				footer: Some(
					format!(
						"用户：{} | 等级：Lv.{} | {sign}",
						profile.nickname, profile.level
					)
					.into(),
				),
				..Default::default()
			}
			.into(),
		])
	}
}
