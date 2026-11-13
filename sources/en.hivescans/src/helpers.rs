use crate::models::Banner;
use aidoku::{
	HomeComponent, HomeComponentValue, HomeLayout, Link, Manga, Result, alloc::Vec,
	imports::html::Document, prelude::*,
};
use iken::{Params, home};
use serde::Deserialize;

pub fn parse_home(html: &Document, params: &Params) -> Result<HomeLayout> {
	let mut layout = home::parse_home(html, params)?;
	// The remaining slides are initialized by the site's script.
	let banners = html
		.select("script")
		.and_then(|scripts| {
			scripts
				.filter_map(|s| s.data().or_else(|| s.html()))
				.find_map(|s| {
					let (_, data) = s.split_once("const hvtXHeroLibrary = ")?;
					Some(Vec::<Banner>::deserialize(
						&mut serde_json::Deserializer::from_str(data),
					))
				})
		})
		.ok_or(error!("Banner data not found"))??;
	let links: Vec<Link> = banners
		.into_iter()
		.take(4)
		.map(|banner| Manga::try_from(banner).map(Link::from))
		.collect::<Result<_>>()?;
	if links.is_empty() {
		bail!("No series banners");
	}
	layout.components.insert(
		0,
		HomeComponent {
			value: HomeComponentValue::ImageScroller {
				links,
				auto_scroll_interval: None,
				width: Some(400),
				height: Some(150),
			},
			..Default::default()
		},
	);
	Ok(layout)
}
