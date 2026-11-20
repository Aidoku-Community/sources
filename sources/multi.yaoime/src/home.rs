use crate::{
	BASE_URL, YaoiMe,
	helpers::{get_html, parse_chapter_list, parse_manga_list},
	models::SiteSettings,
	settings,
};
use aidoku::{
	Home, HomeComponent, HomeComponentValue, HomeLayout, Link, Listing, ListingKind, Result,
	alloc::{String, Vec, format},
	imports::html::Element,
	prelude::*,
};

#[derive(Clone, Copy)]
pub enum Layout {
	Row,
	Chapters,
	Ranked,
}

pub const SECTIONS: [(&str, &str, Layout); 9] = [
	("popular", "Popular Updates", Layout::Row),
	("bookmarked", "Most Bookmarked", Layout::Row),
	("latest", "Latest Releases", Layout::Chapters),
	("trending", "Trending Now", Layout::Row),
	("top-rated", "Top Rated", Layout::Row),
	("rising", "Rising", Layout::Row),
	("talked", "Most Talked About", Layout::Ranked),
	("hidden-gems", "Hidden Gems", Layout::Row),
	("binge", "Binge-Worthy", Layout::Row),
];

// Sections the site's home page shows by default that the source doesn't.
const SITE_SECTIONS: [&str; 4] = ["continue", "reviews", "creators", "about"];

fn component(id: &str, name: &str, layout: Layout, content: &Element) -> HomeComponentValue {
	// Popular updates has no page of its own on the site.
	let listing = (id != "popular").then(|| Listing {
		id: id.into(),
		name: name.into(),
		kind: ListingKind::Default,
	});
	let links = || {
		parse_manga_list(content)
			.into_iter()
			.map(Link::from)
			.collect()
	};
	match layout {
		Layout::Row => HomeComponentValue::Scroller {
			entries: links(),
			listing,
		},
		Layout::Chapters => HomeComponentValue::MangaChapterList {
			page_size: Some(5),
			entries: parse_chapter_list(content),
			listing,
		},
		Layout::Ranked => HomeComponentValue::MangaList {
			ranking: true,
			page_size: Some(4),
			entries: links(),
			listing,
		},
	}
}

impl Home for YaoiMe {
	fn get_home(&self) -> Result<HomeLayout> {
		let mut site = SiteSettings::current();
		if site.shows_nothing() {
			return Ok(HomeLayout::default());
		}
		let shown = settings::home_sections();
		let shows = |id: &str| {
			shown
				.as_ref()
				.is_none_or(|shown| shown.iter().any(|value| value == id))
		};
		let sections: Vec<_> = SECTIONS.iter().filter(|(id, ..)| shows(id)).collect();
		if sections.is_empty() {
			return Ok(HomeLayout::default());
		}
		// The site renders the sections it is given, in that order, and leaves out the rest.
		site.home = sections
			.iter()
			.map(|(id, ..)| String::from(*id))
			.chain(
				SECTIONS
					.iter()
					.map(|(id, ..)| *id)
					.filter(|id| !shows(id))
					.chain(SITE_SECTIONS)
					.map(|id| format!("-{id}")),
			)
			.collect();

		let document = get_html(BASE_URL, &site)?;
		let headings: Vec<Element> = document
			.select("div.section-head > h2.section-title")
			.into_iter()
			.flatten()
			.collect();
		let components = sections
			.iter()
			.filter_map(|(id, name, layout)| {
				// The site writes its headings in sentence case ("Trending now").
				let heading = headings.iter().find(|heading| {
					heading
						.text()
						.is_some_and(|text| text.eq_ignore_ascii_case(name))
				})?;
				let content = heading.parent()?.next()?;
				Some(HomeComponent {
					title: Some(String::from(*name)),
					subtitle: None,
					value: component(id, name, *layout, &content),
				})
			})
			.collect::<Vec<_>>();
		if components.is_empty() {
			bail!("No home sections");
		}
		Ok(HomeLayout { components })
	}
}
