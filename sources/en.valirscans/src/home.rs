use aidoku::{
	HomeComponent, HomeComponentValue, HomeLayout, Link, Listing, Manga, Result, alloc::Vec,
	imports::net::Request, prelude::*,
};
use vinetheme::{Params, checked, compact, home};

pub fn get_home(params: &Params) -> Result<HomeLayout> {
	let requests = [
		"",
		"/api/series?sort=trending&limit=24&page=1&contentMode=all",
		"/api/series?type=Web+Novel&limit=24&page=1&contentMode=all",
	]
	.into_iter()
	.map(|path| Ok(Request::get(format!("{}{path}", params.base_url))?))
	.collect::<Result<Vec<_>>>()?;
	let [home, trending, novels] = <[_; 3]>::try_from(Request::send_all(requests))
		.map_err(|_| error!("Missing home responses"))?
		.map(|response| checked(response?));
	let layout = layout(
		loaded(home.and_then(|response| home::parse(&response.get_string()?, params))),
		loaded(trending.and_then(|response| home::parse_catalogue(response, params))),
		loaded(novels.and_then(|response| home::parse_catalogue(response, params))),
	);
	if layout.components.is_empty() {
		bail!("No home content found");
	}
	Ok(layout)
}

// A failed section drops out rather than failing the whole page
fn loaded<T: Default>(result: Result<T>) -> T {
	result.unwrap_or_else(|error| {
		println!("Home section failed: {error:?}");
		T::default()
	})
}

pub(super) fn layout(mut home: HomeLayout, trending: Vec<Manga>, novels: Vec<Manga>) -> HomeLayout {
	let mut components = Vec::new();
	if !trending.is_empty() {
		components.push(HomeComponent {
			title: Some("Trending".into()),
			value: HomeComponentValue::BigScroller {
				entries: trending,
				auto_scroll_interval: Some(8.0),
			},
			..Default::default()
		});
	}
	if !novels.is_empty() {
		home.components.push(HomeComponent {
			title: Some("Novels".into()),
			value: HomeComponentValue::Scroller {
				entries: novels.into_iter().map(compact).map(Link::from).collect(),
				listing: Some(Listing {
					id: "type=Web%20Novel".into(),
					name: "Novels".into(),
					..Default::default()
				}),
			},
			..Default::default()
		});
	}
	home::reorder(
		HomeLayout {
			components: components.into_iter().chain(home.components).collect(),
		},
		&[
			("Trending", None),
			("Popular Today", None),
			("Latest Comic Updates", Some(4)),
			("Novels", None),
			("Latest Novel Updates", Some(4)),
			("Editor's Pick", None),
			("Top 10 in All", Some(3)),
		],
	)
}
