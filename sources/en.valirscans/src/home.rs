use aidoku::{
	HomeComponent, HomeComponentValue, HomeLayout, Link, Listing, Manga, Result, alloc::Vec,
	imports::net::Request, prelude::*,
};
use vinetheme::{Params, home};

pub fn get_home(params: &Params) -> Result<HomeLayout> {
	let requests = [
		"",
		"/api/series?sort=trending&limit=24&page=1&contentMode=all",
		"/api/series?type=Web+Novel&limit=24&page=1&contentMode=all",
	]
	.into_iter()
	.map(|path| Ok(Request::get(format!("{}{path}", params.base_url))?))
	.collect::<Result<Vec<_>>>()?;
	let mut responses = Request::send_all(requests).into_iter().map(|response| {
		let response = response?;
		let status = response.status_code();
		if !(200..300).contains(&status) {
			bail!("HTTP {status}");
		}
		Ok(response)
	});
	let home = responses
		.next()
		.unwrap()
		.and_then(|response| home::parse(&response.get_string()?, params));
	let trending = responses
		.next()
		.unwrap()
		.and_then(|response| home::parse_catalogue(response, params));
	let novels = responses
		.next()
		.unwrap()
		.and_then(|response| home::parse_catalogue(response, params));
	for result in [
		&home.as_ref().map(|_| ()),
		&trending.as_ref().map(|_| ()),
		&novels.as_ref().map(|_| ()),
	] {
		if let Err(error) = result {
			println!("Home section failed: {error:?}");
		}
	}
	let layout = layout(
		home.unwrap_or_default(),
		trending.unwrap_or_default(),
		novels.unwrap_or_default(),
	);
	if layout.components.is_empty() {
		bail!("No home content found");
	}
	Ok(layout)
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
				entries: novels
					.into_iter()
					.map(|manga| {
						Link::from(Manga {
							key: manga.key,
							title: manga.title,
							cover: manga.cover,
							content_rating: manga.content_rating,
							..Default::default()
						})
					})
					.collect(),
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
