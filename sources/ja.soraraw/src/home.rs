use crate::{
	BASE_URL, Soraraw,
	helpers::request,
	models::{DataProps, HomeData, MangaEntry, Props, TopList},
};
use aidoku::{
	Home, HomeComponent, HomeComponentValue, HomeLayout, Link, Listing, ListingKind, Manga, Result,
	alloc::{Vec, vec},
	imports::net::{Request, RequestError, Response},
	prelude::*,
};

impl Home for Soraraw {
	fn get_home(&self) -> Result<HomeLayout> {
		let responses: [core::result::Result<Response, RequestError>; 3] = Request::send_all([
			request(format!("{BASE_URL}/_next/data/soraraw38/index.json"))?,
			request(format!("{BASE_URL}/top/today.json"))?,
			request(format!("{BASE_URL}/top/rising.json"))?,
		])
		.try_into()
		.expect("three home requests");
		let [home, today, rising] = responses;
		let home = home?
			.get_json_owned::<Props<DataProps<HomeData>>>()?
			.page_props
			.data;
		let today = today?.get_json_owned::<TopList>()?;
		let rising = rising?.get_json_owned::<TopList>()?;

		Ok(HomeLayout {
			components: vec![
				HomeComponent {
					title: None,
					subtitle: None,
					value: HomeComponentValue::BigScroller {
						entries: home
							.hot
							.into_iter()
							.map(MangaEntry::into_full_manga)
							.take(10)
							.collect(),
						auto_scroll_interval: Some(8.0),
					},
				},
				HomeComponent {
					title: Some("最新更新".into()),
					subtitle: Some("毎日更新の最新マンガ".into()),
					value: scroller(
						home.results.into_iter().map(Manga::from).collect(),
						"newest",
						"新着",
					),
				},
				HomeComponent {
					title: Some("24時間ランキング".into()),
					subtitle: Some("今日もっとも読まれた作品".into()),
					value: scroller(
						today.mangas.into_iter().map(Manga::from).collect(),
						"trending",
						"ランキング",
					),
				},
				HomeComponent {
					title: Some("今週急上昇".into()),
					subtitle: Some("勢いのある作品".into()),
					value: scroller(
						rising
							.mangas
							.into_iter()
							.take(20)
							.map(Manga::from)
							.collect(),
						"rising",
						"急上昇",
					),
				},
			],
		})
	}
}

fn scroller(entries: Vec<Manga>, id: &str, name: &str) -> HomeComponentValue {
	HomeComponentValue::Scroller {
		entries: entries.into_iter().map(Link::from).collect(),
		listing: Some(Listing {
			id: id.into(),
			name: name.into(),
			kind: ListingKind::Default,
		}),
	}
}
