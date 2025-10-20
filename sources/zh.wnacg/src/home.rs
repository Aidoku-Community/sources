use crate::{BASE_URL, USER_AGENT, Wnacg, html::MangaPage as _};
use aidoku::{
	AidokuError, Home, HomeComponent, HomeLayout, HomePartialResult, Listing, ListingKind, Manga,
	Result,
	alloc::{Vec, vec},
	imports::{
		net::{Request, RequestError, Response},
		std::send_partial_result,
	},
	prelude::*,
};

impl Home for Wnacg {
	fn get_home(&self) -> Result<HomeLayout> {
		// send basic home layout
		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some("日排行".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_big_scroller(),
				},
				HomeComponent {
					title: Some("周排行".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_manga_list(),
				},
				HomeComponent {
					title: Some("月排行".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_manga_list(),
				},
				HomeComponent {
					title: Some("最近更新".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("同人志".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("单行本".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("杂志&短篇".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("韩漫".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_scroller(),
				},
			],
		}));

		let responses: [core::result::Result<Response, RequestError>; 8] = Request::send_all([
			// daily ranking
			Request::get(format!(
				"{}/albums-favorite_ranking-page-1-type-day",
				BASE_URL
			))?
			.header("User-Agent", USER_AGENT)
			.header("Origin", BASE_URL),
			// weekly ranking
			Request::get(format!(
				"{}/albums-favorite_ranking-page-1-type-week",
				BASE_URL
			))?
			.header("User-Agent", USER_AGENT)
			.header("Origin", BASE_URL),
			// monthly ranking
			Request::get(format!(
				"{}/albums-favorite_ranking-page-1-type-month",
				BASE_URL
			))?
			.header("User-Agent", USER_AGENT)
			.header("Origin", BASE_URL),
			// latest updates
			Request::get(format!("{}/albums-index-page-1.html", BASE_URL))?
				.header("User-Agent", USER_AGENT)
				.header("Origin", BASE_URL),
			// doujinshi (同人志)
			Request::get(format!("{}/albums-index-page-1-cate-5.html", BASE_URL))?
				.header("User-Agent", USER_AGENT)
				.header("Origin", BASE_URL),
			// single volume (单行本)
			Request::get(format!("{}/albums-index-page-1-cate-6.html", BASE_URL))?
				.header("User-Agent", USER_AGENT)
				.header("Origin", BASE_URL),
			// magazine & short stories (杂志&短篇)
			Request::get(format!("{}/albums-index-page-1-cate-7.html", BASE_URL))?
				.header("User-Agent", USER_AGENT)
				.header("Origin", BASE_URL),
			// korean manhwa (韩漫)
			Request::get(format!("{}/albums-index-page-1-cate-19.html", BASE_URL))?
				.header("User-Agent", USER_AGENT)
				.header("Origin", BASE_URL),
		])
		.try_into()
		.or_else(|_| {
			Err(AidokuError::message(
				"Failed to convert requests vec to array",
			))
		})?;
		let results: [Result<Vec<Manga>>; 8] = responses
			.map(|res| res?.get_html()?.manga_page_result())
			.map(|res| Ok(res?.entries));

		let [
			daily,
			weekly,
			monthly,
			latest,
			doujinshi,
			single_volume,
			magazine,
			korean,
		] = results;
		let daily = daily?;
		let weekly = weekly?;
		let monthly = monthly?;
		let latest = latest?;
		let doujinshi = doujinshi?;
		let single_volume = single_volume?;
		let magazine = magazine?;
		let korean = korean?;

		let mut components = Vec::new();

		if !daily.is_empty() {
			components.push(HomeComponent {
				title: Some("日排行".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::BigScroller {
					entries: daily,
					auto_scroll_interval: Some(8.0),
				},
			});
		}

		if !weekly.is_empty() {
			components.push(HomeComponent {
				title: Some("周排行".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::MangaList {
					ranking: true,
					page_size: Some(3),
					entries: weekly.into_iter().map(|manga| manga.into()).collect(),
					listing: Some(Listing {
						id: "weekup".into(),
						name: "周排行".into(),
						kind: ListingKind::Default,
					}),
				},
			});
		}

		if !monthly.is_empty() {
			components.push(HomeComponent {
				title: Some("月排行".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::MangaList {
					ranking: true,
					page_size: Some(3),
					entries: monthly.into_iter().map(|manga| manga.into()).collect(),
					listing: Some(Listing {
						id: "monthup".into(),
						name: "月排行".into(),
						kind: ListingKind::Default,
					}),
				},
			});
		}

		if !latest.is_empty() {
			components.push(HomeComponent {
				title: Some("最近更新".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::Scroller {
					entries: latest.into_iter().map(|manga| manga.into()).collect(),
					listing: Some(Listing {
						id: "update".into(),
						name: "最近更新".into(),
						kind: ListingKind::Default,
					}),
				},
			});
		}

		if !doujinshi.is_empty() {
			components.push(HomeComponent {
				title: Some("同人志".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::Scroller {
					entries: doujinshi.into_iter().map(|manga| manga.into()).collect(),
					listing: Some(Listing {
						id: "doujinshi".into(),
						name: "同人志".into(),
						kind: ListingKind::Default,
					}),
				},
			});
		}

		if !single_volume.is_empty() {
			components.push(HomeComponent {
				title: Some("单行本".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::Scroller {
					entries: single_volume
						.into_iter()
						.map(|manga| manga.into())
						.collect(),
					listing: Some(Listing {
						id: "single-volume".into(),
						name: "单行本".into(),
						kind: ListingKind::Default,
					}),
				},
			});
		}

		if !magazine.is_empty() {
			components.push(HomeComponent {
				title: Some("杂志&短篇".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::Scroller {
					entries: magazine.into_iter().map(|manga| manga.into()).collect(),
					listing: Some(Listing {
						id: "magazine".into(),
						name: "杂志&短篇".into(),
						kind: ListingKind::Default,
					}),
				},
			});
		}

		if !korean.is_empty() {
			components.push(HomeComponent {
				title: Some("韩漫".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::Scroller {
					entries: korean.into_iter().map(|manga| manga.into()).collect(),
					listing: Some(Listing {
						id: "korean".into(),
						name: "韩漫".into(),
						kind: ListingKind::Default,
					}),
				},
			});
		}

		Ok(HomeLayout { components })
	}
}
