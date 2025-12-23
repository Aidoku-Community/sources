use crate::{
	models::NHentaiSearchResponse, normalize_blocklist, settings, NHentai, API_URL,
};
use aidoku::{
	alloc::{vec, Vec},
	helpers::uri::encode_uri_component,
	imports::{
		net::{Request, RequestError, Response},
		std::send_partial_result,
	},
	prelude::*,
	Home, HomeComponent, HomeLayout, HomePartialResult, Listing, ListingKind, Manga, Result,
};

impl Home for NHentai {
	fn get_home(&self) -> Result<HomeLayout> {
		// send basic home layout
		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some("Popular Today".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_big_scroller(),
				},
				HomeComponent {
					title: Some("Popular This Week".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_manga_list(),
				},
				HomeComponent {
					title: Some("Popular All Time".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_manga_list(),
				},
				HomeComponent {
					title: Some("Latest".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_scroller(),
				},
			],
		}));

		let blocklist = normalize_blocklist(settings::get_blocklist());
		let query = encode_uri_component(
			settings::get_language()
				.map(|language| format!("language:{language}"))
				.unwrap_or(" ".into()),
		);

		let responses: [core::result::Result<Response, RequestError>; 4] = Request::send_all([
			// popular today
			Request::get(format!(
				"{API_URL}/galleries/search?query={query}&page=1&sort=popular-today"
			))?,
			// popular week
			Request::get(format!(
				"{API_URL}/galleries/search?query={query}&page=1&sort=popular-week"
			))?,
			// popular all
			Request::get(format!(
				"{API_URL}/galleries/search?query={query}&page=1&sort=popular"
			))?,
			// latest
			Request::get(format!(
				"{API_URL}/galleries/search?query={query}&page=1&sort=recent"
			))?,
		])
		.try_into()
		.expect("requests vec length should be 4");
		let results: [Result<Vec<Manga>>; 4] = responses
			.map(|res| res?.get_json::<NHentaiSearchResponse>())
			.map(|res| {
				Ok(res?
					.result
					.into_iter()
					.filter(|gallery| {
						if blocklist.is_empty() {
							return true;
						}
						!gallery
							.tags
							.iter()
							.any(|tag| blocklist.contains(&tag.name.to_lowercase()))
					})
					.map(|gallery| gallery.into())
					.collect::<Vec<Manga>>())
			});
		let [popular_today, popular_week, popular_all, recent] = results;
		let popular_today = popular_today?;
		let popular_week = popular_week?;
		let popular_all = popular_all?;
		let recent = recent?;

		let mut components = Vec::new();

		if !popular_today.is_empty() {
			components.push(HomeComponent {
				title: Some("Popular Today".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::BigScroller {
					entries: popular_today,
					auto_scroll_interval: Some(8.0),
				},
			});
		}

		if !popular_week.is_empty() {
			components.push(HomeComponent {
				title: Some("Popular This Week".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::MangaList {
					ranking: true,
					page_size: Some(3),
					entries: popular_week.into_iter().map(|item| item.into()).collect(),
					listing: Some(Listing {
						id: "popular-week".into(),
						name: "Popular This Week".into(),
						kind: if settings::get_list_viewer() {
							ListingKind::List
						} else {
							ListingKind::Default
						},
					}),
				},
			});
		}

		if !popular_all.is_empty() {
			components.push(HomeComponent {
				title: Some("Popular All Time".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::MangaList {
					ranking: true,
					page_size: Some(3),
					entries: popular_all.into_iter().map(|item| item.into()).collect(),
					listing: Some(Listing {
						id: "popular".into(),
						name: "Popular All Time".into(),
						kind: if settings::get_list_viewer() {
							ListingKind::List
						} else {
							ListingKind::Default
						},
					}),
				},
			});
		}

		if !recent.is_empty() {
			components.push(HomeComponent {
				title: Some("Latest".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::Scroller {
					entries: recent.into_iter().map(|item| item.into()).collect(),
					listing: Some(Listing {
						id: "latest".into(),
						name: "Latest".into(),
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
