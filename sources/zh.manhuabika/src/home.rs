use crate::{Manhuabika, json::*, net, settings};
use aidoku::{
	AidokuError, Home, HomeComponent, HomeLayout, HomePartialResult, Listing, ListingKind,
	MangaPageResult, Result,
	alloc::{String, Vec, vec},
	imports::{
		net::{Request, RequestError, Response},
		std::send_partial_result,
	},
};

impl Home for Manhuabika {
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

		let requests = [
			// daily ranking
			net::create_request(net::gen_rank_url("H24".into()), "GET", String::new()),
			// weekly ranking
			net::create_request(net::gen_rank_url("D7".into()), "GET", String::new()),
			// monthly ranking
			net::create_request(net::gen_rank_url("D30".into()), "GET", String::new()),
			// 大家都在看
			net::create_request(
				net::gen_explore_url("大家都在看".into(), "dd".into(), 1),
				"GET",
				String::new(),
			),
			// 官方都在看
			net::create_request(
				net::gen_explore_url("官方都在看".into(), "dd".into(), 1),
				"GET",
				String::new(),
			),
			// 大湿推荐
			net::create_request(
				net::gen_explore_url("大濕推薦".into(), "dd".into(), 1),
				"GET",
				String::new(),
			),
			// 那年今天
			net::create_request(
				net::gen_explore_url("那年今天".into(), "dd".into(), 1),
				"GET",
				String::new(),
			),
			// latest updates
			net::create_request(
				net::gen_explore_url("".into(), "dd".into(), 1),
				"GET",
				String::new(),
			),
		];

		let requests: [Request; 8] = requests
			.into_iter()
			.map(|r| {
				r.or_else(|_| {
					Err(AidokuError::message(
						"Failed to create request, please check login status",
					))
				})
			})
			.collect::<Result<Vec<Request>>>()?
			.try_into()
			.or_else(|_| Err(AidokuError::message("Failed to convert requests to array")))?;

		let responses: [core::result::Result<Response, RequestError>; 8] =
			Request::send_all(requests)
				.try_into()
				.or_else(|_| Err(AidokuError::message("Failed to convert responses to array")))?;

		let results: [Result<MangaPageResult>; 8] = responses.map(|res| {
			let json: serde_json::Value = res?.get_json()?;
			let data = json
				.get("data")
				.ok_or(AidokuError::message("No data in response"))?;

			// Handle different response formats
			if let Some(comics_obj) = data.get("comics").and_then(|c| c.as_object()) {
				// Explore format
				let explore_data = ExploreData {
					comics: ComicsData {
						docs: comics_obj
							.get("docs")
							.and_then(|d| d.as_array())
							.ok_or(AidokuError::message("No docs in comics"))?
							.iter()
							.map(|item| {
								serde_json::from_value::<ComicItem>(serde_json::Value::Object(
									item.as_object()
										.ok_or_else(|| AidokuError::message("Invalid item format"))?
										.clone(),
								))
								.or_else(|_| {
									Err(AidokuError::message("Failed to parse comic item"))
								})
							})
							.collect::<Result<Vec<ComicItem>>>()?,
						page: comics_obj.get("page").and_then(|p| p.as_i64()).unwrap_or(1) as i32,
						pages: comics_obj
							.get("pages")
							.and_then(|p| p.as_i64())
							.unwrap_or(1) as i32,
					},
				};
				Ok(explore_data.into())
			} else {
				// Rank format
				let rank_data = RankData {
					comics: data
						.get("comics")
						.and_then(|c| c.as_array())
						.ok_or(AidokuError::message("No comics in data"))?
						.iter()
						.map(|item| {
							serde_json::from_value::<ComicItem>(serde_json::Value::Object(
								item.as_object()
									.ok_or_else(|| AidokuError::message("Invalid item format"))?
									.clone(),
							))
							.or_else(|_| Err(AidokuError::message("Failed to parse comic item")))
						})
						.collect::<Result<Vec<ComicItem>>>()?,
				};
				Ok(rank_data.into())
			}
		});

		let [
			daily,
			weekly,
			monthly,
			popular,
			official,
			recommended,
			today_in_history,
			latest,
		] = results;
		let daily = daily?;
		let weekly = weekly?;
		let monthly = monthly?;
		let popular = popular?;
		let official = official?;
		let recommended = recommended?;
		let today_in_history = today_in_history?;
		let latest = latest?;

		let mut components = Vec::new();

		if !daily.entries.is_empty() {
			components.push(HomeComponent {
				title: Some("日榜".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::BigScroller {
					entries: daily.entries,
					auto_scroll_interval: Some(8.0),
				},
			});
		}

		if !weekly.entries.is_empty() {
			components.push(HomeComponent {
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

		if !monthly.entries.is_empty() {
			components.push(HomeComponent {
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

		if !popular.entries.is_empty() {
			components.push(HomeComponent {
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

		if !official.entries.is_empty() {
			components.push(HomeComponent {
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

		if !recommended.entries.is_empty() {
			components.push(HomeComponent {
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

		if !today_in_history.entries.is_empty() {
			components.push(HomeComponent {
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

		if !latest.entries.is_empty() {
			components.push(HomeComponent {
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

		Ok(HomeLayout { components })
	}
}
