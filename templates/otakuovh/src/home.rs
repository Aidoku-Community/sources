use aidoku::{
	HomeComponent, HomeComponentValue, HomeLayout, HomePartialResult, Result,
	alloc::{Vec, string::ToString, vec},
	imports::{net::Request, std::send_partial_result},
};

use crate::{
	Params, endpoints::Url, json::ResponseJsonExt, models::manga::InkManga, request::InkRequest,
};

const EDITORS_CHOICE: &str = "Выбор редакции";
const POPULAR_TITLE: &str = "Популярные онгоинги";
const BEST_COMPLETED: &str = "Лучшие завершенные";
const RECENTLY_ADDED: &str = "Недавно добавлено";

pub fn initial_layout() {
	let components = vec![
		HomeComponent {
			title: Some(EDITORS_CHOICE.to_string()),
			subtitle: None,
			value: HomeComponentValue::empty_scroller(),
		},
		HomeComponent {
			title: Some(POPULAR_TITLE.to_string()),
			subtitle: None,
			value: HomeComponentValue::empty_scroller(),
		},
		HomeComponent {
			title: Some(BEST_COMPLETED.to_string()),
			subtitle: None,
			value: HomeComponentValue::empty_scroller(),
		},
		HomeComponent {
			title: Some(RECENTLY_ADDED.to_string()),
			subtitle: None,
			value: HomeComponentValue::empty_scroller(),
		},
	];

	send_partial_result(&HomePartialResult::Layout(HomeLayout { components }))
}

pub fn load_editors_choice(params: &Params) -> Result<()> {
	let search_params = vec![
		("featured".to_string(), "1".to_string()),
		("page".to_string(), "0".to_string()),
		("size".to_string(), "20".to_string()),
	];
	let url = Url::manga_search_with_params(&params.base_url, search_params);
	let response = Request::get(&url)?
		.prepared_headers(params)?
		.parse_json::<Vec<InkManga>>()?;

	send_partial_result(&HomePartialResult::Component(HomeComponent {
		title: Some(EDITORS_CHOICE.into()),
		subtitle: None,
		value: HomeComponentValue::Scroller {
			entries: response
				.into_iter()
				.map(|manga| manga.into_link())
				.collect(),
			listing: None,
		},
	}));

	Ok(())
}

pub fn load_popular_ongoings(params: &Params) -> Result<()> {
	let url = Url::manga_search_with_params(
		&params.base_url,
		[
			("status", "ONGOING"),
			("page", "0"),
			("size", "20"),
			("sort", "viewsCount,desc"),
		]
		.into_iter()
		.map(|(key, value)| (key.to_string(), value.to_string()))
		.collect(),
	);
	let response = Request::get(&url)?
		.prepared_headers(params)?
		.parse_json::<Vec<InkManga>>()?;

	send_partial_result(&HomePartialResult::Component(HomeComponent {
		title: Some(POPULAR_TITLE.into()),
		subtitle: None,
		value: HomeComponentValue::Scroller {
			entries: response
				.into_iter()
				.map(|manga| manga.into_link())
				.collect(),
			listing: None,
		},
	}));

	Ok(())
}

pub fn load_best_completed(params: &Params) -> Result<()> {
	let url = Url::manga_search_with_params(
		&params.base_url,
		[
			("status", "DONE"),
			("page", "0"),
			("size", "20"),
			("sort", "viewsCount,desc"),
		]
		.into_iter()
		.map(|(key, value)| (key.to_string(), value.to_string()))
		.collect(),
	);
	let response = Request::get(&url)?
		.prepared_headers(params)?
		.parse_json::<Vec<InkManga>>()?;

	send_partial_result(&HomePartialResult::Component(HomeComponent {
		title: Some(BEST_COMPLETED.into()),
		subtitle: None,
		value: HomeComponentValue::Scroller {
			entries: response
				.into_iter()
				.map(|manga| manga.into_link())
				.collect(),
			listing: None,
		},
	}));

	Ok(())
}

pub fn load_recently_added(params: &Params) -> Result<()> {
	let url = Url::manga_search_with_params(
		&params.base_url,
		[("page", "0"), ("size", "20"), ("sort", "createdAt,desc")]
			.into_iter()
			.map(|(key, value)| (key.to_string(), value.to_string()))
			.collect(),
	);
	let response = Request::get(&url)?
		.prepared_headers(params)?
		.parse_json::<Vec<InkManga>>()?;

	send_partial_result(&HomePartialResult::Component(HomeComponent {
		title: Some(RECENTLY_ADDED.into()),
		subtitle: None,
		value: HomeComponentValue::Scroller {
			entries: response
				.into_iter()
				.map(|manga| manga.into_link())
				.collect(),
			listing: None,
		},
	}));

	Ok(())
}

