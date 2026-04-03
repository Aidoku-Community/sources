use aidoku::{
	Home, HomeComponent, HomeComponentValue, HomeLayout, Link, Listing, ListingKind, Manga,
	MangaWithChapter, Result,
	alloc::Vec,
	imports::net::{Request, RequestError, Response},
	prelude::*,
};

use crate::helpers::{build_form_body, get_adult_mode, get_base_url, post_with_form};
use crate::models::ListingResp;
use crate::{NoyAcg, auth};

impl Home for NoyAcg {
	fn get_home(&self) -> Result<HomeLayout> {
		auth::ensure_session()?;
		auth::try_daily_signin();
		if !auth::is_logged_in() {
			bail!("請先登入以檢視內容");
		}
		let adult = get_adult_mode();
		let base_url = get_base_url();
		let referer = format!("{base_url}/");

		let views_body = build_form_body(&[("page", "1"), ("sort", "views")]);
		let new_body = build_form_body(&[("page", "1"), ("sort", "new")]);
		let read_day_body = build_form_body(&[("type", "day"), ("page", "1")]);
		let read_week_body = build_form_body(&[("type", "week"), ("page", "1")]);
		let read_month_body = build_form_body(&[("type", "moon"), ("page", "1")]);

		let responses: Vec<core::result::Result<Response, RequestError>> = Request::send_all([
			post_with_form(
				&format!("{base_url}/api/b1/booklist"),
				&views_body,
				&referer,
				&adult,
			)?,
			post_with_form(
				&format!("{base_url}/api/b1/booklist"),
				&new_body,
				&referer,
				&adult,
			)?,
			post_with_form(
				&format!("{base_url}/api/readLeaderboard"),
				&read_day_body,
				&referer,
				&adult,
			)?,
			post_with_form(
				&format!("{base_url}/api/readLeaderboard"),
				&read_week_body,
				&referer,
				&adult,
			)?,
			post_with_form(
				&format!("{base_url}/api/readLeaderboard"),
				&read_month_body,
				&referer,
				&adult,
			)?,
			Request::post(format!("{base_url}/api/v4/book/random"))?
				.header("Referer", &referer)
				.header("allow-adult", &adult),
		]);

		let mut iter = responses.into_iter();
		let views_resp = iter.next();
		let new_resp = iter.next();
		let read_day_resp = iter.next();
		let read_week_resp = iter.next();
		let read_month_resp = iter.next();
		let random_resp = iter.next();

		let mut components: Vec<HomeComponent> = Vec::new();

		if let Some(manga) = views_resp.and_then(parse_listing_full) {
			let entries: Vec<Manga> = manga.into_iter().take(10).collect();
			if !entries.is_empty() {
				components.push(HomeComponent {
					title: Some("熱門推薦".into()),
					subtitle: None,
					value: HomeComponentValue::BigScroller {
						entries,
						auto_scroll_interval: Some(8.0),
					},
				});
			}
		}

		if let Some(entries) = new_resp.and_then(parse_listing_chapter) {
			components.push(HomeComponent {
				title: Some("最新內容".into()),
				subtitle: None,
				value: HomeComponentValue::MangaChapterList {
					page_size: Some(4),
					entries,
					listing: Some(Listing {
						id: "latest".into(),
						name: "更新".into(),
						kind: ListingKind::Default,
					}),
				},
			});
		}

		if let Some(manga) = random_resp.and_then(parse_listing_random) {
			let entries: Vec<Link> = manga.into_iter().map(Into::into).collect();
			if !entries.is_empty() {
				components.push(HomeComponent {
					title: Some("隨機推薦".into()),
					subtitle: None,
					value: HomeComponentValue::Scroller {
						entries,
						listing: None,
					},
				});
			}
		}

		if let Some(manga) = read_day_resp.and_then(parse_listing_basic) {
			let entries: Vec<Link> = manga.into_iter().map(Into::into).collect();
			if !entries.is_empty() {
				components.push(HomeComponent {
					title: Some("日閱讀榜".into()),
					subtitle: None,
					value: HomeComponentValue::Scroller {
						entries,
						listing: Some(Listing {
							id: "leaderboard:day".into(),
							name: "日閱讀榜".into(),
							kind: ListingKind::Default,
						}),
					},
				});
			}
		}

		if let Some(manga) = read_week_resp.and_then(parse_listing_basic) {
			let entries: Vec<Link> = manga.into_iter().map(Into::into).collect();
			if !entries.is_empty() {
				components.push(HomeComponent {
					title: Some("週閱讀榜".into()),
					subtitle: None,
					value: HomeComponentValue::Scroller {
						entries,
						listing: Some(Listing {
							id: "leaderboard:week".into(),
							name: "週閱讀榜".into(),
							kind: ListingKind::Default,
						}),
					},
				});
			}
		}

		if let Some(manga) = read_month_resp.and_then(parse_listing_basic) {
			let entries: Vec<Link> = manga.into_iter().map(Into::into).collect();
			if !entries.is_empty() {
				components.push(HomeComponent {
					title: Some("月閱讀榜".into()),
					subtitle: None,
					value: HomeComponentValue::Scroller {
						entries,
						listing: Some(Listing {
							id: "leaderboard:moon".into(),
							name: "月閱讀榜".into(),
							kind: ListingKind::Default,
						}),
					},
				});
			}
		}

		if components.is_empty() {
			bail!("無法取得資料，請嘗試切換分流伺服器");
		}
		Ok(HomeLayout { components })
	}
}

fn parse_listing_full(result: core::result::Result<Response, RequestError>) -> Option<Vec<Manga>> {
	let resp: ListingResp = result.ok()?.get_json_owned().ok()?;
	Some(resp.info?.into_iter().map(Into::into).collect())
}

fn parse_listing_basic(result: core::result::Result<Response, RequestError>) -> Option<Vec<Manga>> {
	let resp: ListingResp = result.ok()?.get_json_owned().ok()?;
	Some(
		resp.info?
			.into_iter()
			.map(|m| m.into_basic_manga())
			.collect(),
	)
}

fn parse_listing_chapter(
	result: core::result::Result<Response, RequestError>,
) -> Option<Vec<MangaWithChapter>> {
	let resp: ListingResp = result.ok()?.get_json_owned().ok()?;
	let entries = resp.into_manga_chapter_list();
	if entries.is_empty() {
		None
	} else {
		Some(entries)
	}
}

fn parse_listing_random(
	result: core::result::Result<Response, RequestError>,
) -> Option<Vec<Manga>> {
	let resp: ListingResp = result.ok()?.get_json_owned().ok()?;
	// random api returns items in "data" instead of "info"
	Some(
		resp.data
			.or(resp.info)?
			.into_iter()
			.map(|m| m.into_basic_manga())
			.collect(),
	)
}
