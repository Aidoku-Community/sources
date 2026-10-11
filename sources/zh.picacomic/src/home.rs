use crate::{Picacomic, json::*, net, settings};
use aidoku::{
	Home, HomeComponent, HomeLayout, HomePartialResult, Listing, ListingKind, MangaPageResult,
	Result,
	alloc::{Vec, vec},
	error,
	imports::{
		net::{HttpMethod, Request, RequestError, Response},
		std::send_partial_result,
	},
	prelude::*,
};

fn send_component(component: HomeComponent) {
	send_partial_result(&HomePartialResult::Component(component));
}

impl Home for Picacomic {
	fn get_home(&self) -> Result<HomeLayout> {
		// send basic home layout
		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some("日榜".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_big_scroller(),
				},
				HomeComponent {
					title: Some("周榜".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_manga_list(),
				},
				HomeComponent {
					title: Some("月榜".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_manga_list(),
				},
				HomeComponent {
					title: Some("大家都在看".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("官方都在看".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("大湿推荐".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("那年今天".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("最近更新".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_scroller(),
				},
			],
		}));

		let build_requests = || -> Result<[Request; 8]> {
			let requests = [
				// daily ranking
				net::create_request(net::gen_rank_url("H24".into()), HttpMethod::Get, None),
				// weekly ranking
				net::create_request(net::gen_rank_url("D7".into()), HttpMethod::Get, None),
				// monthly ranking
				net::create_request(net::gen_rank_url("D30".into()), HttpMethod::Get, None),
				// 大家都在看
				net::create_request(
					net::gen_explore_url("大家都在看".into(), "dd".into(), 1),
					HttpMethod::Get,
					None,
				),
				// 官方都在看
				net::create_request(
					net::gen_explore_url("官方都在看".into(), "dd".into(), 1),
					HttpMethod::Get,
					None,
				),
				// 大湿推荐
				net::create_request(
					net::gen_explore_url("大濕推薦".into(), "dd".into(), 1),
					HttpMethod::Get,
					None,
				),
				// 那年今天
				net::create_request(
					net::gen_explore_url("那年今天".into(), "dd".into(), 1),
					HttpMethod::Get,
					None,
				),
				// latest updates
				net::create_request(
					net::gen_explore_url("".into(), "dd".into(), 1),
					HttpMethod::Get,
					None,
				),
			];
			requests
				.into_iter()
				.map(|r| {
					r.map_err(|_| error!("Failed to create request, please check login status"))
				})
				.collect::<Result<Vec<Request>>>()?
				.try_into()
				.map_err(|_| error!("Failed to convert requests to array"))
		};

		let mut responses: Vec<core::result::Result<Response, RequestError>> =
			Request::send_all(build_requests()?);

		if responses.len() != 8 {
			bail!("Failed to get all responses");
		}

		// If any response is 401, re-login and retry all requests
		if responses
			.iter()
			.any(|r| r.as_ref().is_ok_and(|resp| resp.status_code() == 401))
		{
			net::login()?;
			responses = Request::send_all(build_requests()?);
			if responses.len() != 8 {
				bail!("Failed to get all responses");
			}
		}

		// responses[0..3] are rank format, responses[3..8] are explore format
		let parse_rank =
			|res: core::result::Result<Response, RequestError>| -> Result<MangaPageResult> {
				let rank_response: RankResponse = res?.get_json_owned()?;
				Ok(rank_response.data.into())
			};
		let parse_explore =
			|res: core::result::Result<Response, RequestError>| -> Result<MangaPageResult> {
				let explore_response: ExploreResponse = res?.get_json_owned()?;
				Ok(explore_response.data.into())
			};

		let r7 = parse_explore(responses.remove(7));
		let r6 = parse_explore(responses.remove(6));
		let r5 = parse_explore(responses.remove(5));
		let r4 = parse_explore(responses.remove(4));
		let r3 = parse_explore(responses.remove(3));
		let r2 = parse_rank(responses.remove(2));
		let r1 = parse_rank(responses.remove(1));
		let r0 = parse_rank(responses.remove(0));

		let [
			daily,
			weekly,
			monthly,
			popular,
			official,
			recommended,
			today_in_history,
			latest,
		] = [r0, r1, r2, r3, r4, r5, r6, r7];

		if let Ok(daily) = daily
			&& !daily.entries.is_empty()
		{
			send_component(HomeComponent {
				title: Some("日榜".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::BigScroller {
					entries: daily.entries,
					auto_scroll_interval: Some(8.0),
				},
			});
		}

		if let Ok(weekly) = weekly
			&& !weekly.entries.is_empty()
		{
			send_component(HomeComponent {
				title: Some("周榜".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::MangaList {
					ranking: true,
					page_size: Some(3),
					entries: weekly
						.entries
						.into_iter()
						.map(|manga| manga.into())
						.collect(),
					listing: Some(Listing {
						id: "weekup".into(),
						name: "周榜".into(),
						kind: if settings::get_list_viewer() {
							ListingKind::List
						} else {
							ListingKind::Default
						},
					}),
				},
			});
		}

		if let Ok(monthly) = monthly
			&& !monthly.entries.is_empty()
		{
			send_component(HomeComponent {
				title: Some("月榜".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::MangaList {
					ranking: true,
					page_size: Some(3),
					entries: monthly
						.entries
						.into_iter()
						.map(|manga| manga.into())
						.collect(),
					listing: Some(Listing {
						id: "monthup".into(),
						name: "月榜".into(),
						kind: if settings::get_list_viewer() {
							ListingKind::List
						} else {
							ListingKind::Default
						},
					}),
				},
			});
		}

		if let Ok(popular) = popular
			&& !popular.entries.is_empty()
		{
			send_component(HomeComponent {
				title: Some("大家都在看".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::Scroller {
					entries: popular
						.entries
						.into_iter()
						.map(|manga| manga.into())
						.collect(),
					listing: Some(Listing {
						id: "djkz".into(),
						name: "大家都在看".into(),
						kind: if settings::get_list_viewer() {
							ListingKind::List
						} else {
							ListingKind::Default
						},
					}),
				},
			});
		}

		if let Ok(official) = official
			&& !official.entries.is_empty()
		{
			send_component(HomeComponent {
				title: Some("官方都在看".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::Scroller {
					entries: official
						.entries
						.into_iter()
						.map(|manga| manga.into())
						.collect(),
					listing: Some(Listing {
						id: "gfdjkz".into(),
						name: "官方都在看".into(),
						kind: if settings::get_list_viewer() {
							ListingKind::List
						} else {
							ListingKind::Default
						},
					}),
				},
			});
		}

		if let Ok(recommended) = recommended
			&& !recommended.entries.is_empty()
		{
			send_component(HomeComponent {
				title: Some("大湿推荐".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::Scroller {
					entries: recommended
						.entries
						.into_iter()
						.map(|manga| manga.into())
						.collect(),
					listing: Some(Listing {
						id: "dswj".into(),
						name: "大湿推荐".into(),
						kind: if settings::get_list_viewer() {
							ListingKind::List
						} else {
							ListingKind::Default
						},
					}),
				},
			});
		}

		if let Ok(today_in_history) = today_in_history
			&& !today_in_history.entries.is_empty()
		{
			send_component(HomeComponent {
				title: Some("那年今天".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::Scroller {
					entries: today_in_history
						.entries
						.into_iter()
						.map(|manga| manga.into())
						.collect(),
					listing: Some(Listing {
						id: "nndtn".into(),
						name: "那年今天".into(),
						kind: if settings::get_list_viewer() {
							ListingKind::List
						} else {
							ListingKind::Default
						},
					}),
				},
			});
		}

		if let Ok(latest) = latest
			&& !latest.entries.is_empty()
		{
			send_component(HomeComponent {
				title: Some("最近更新".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::Scroller {
					entries: latest
						.entries
						.into_iter()
						.map(|manga| manga.into())
						.collect(),
					listing: Some(Listing {
						id: "update".into(),
						name: "最近更新".into(),
						kind: if settings::get_list_viewer() {
							ListingKind::List
						} else {
							ListingKind::Default
						},
					}),
				},
			});
		}

		Ok(HomeLayout::default())
	}
}
