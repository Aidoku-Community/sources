use crate::{
	Toonkotsu,
	api::{self, BrowseParams, browse_request, discovery_request, latest_request, ranking_request},
	helpers::allows,
	models::{LatestList, Series},
	settings,
};
use aidoku::{
	Home, HomeComponent, HomeComponentValue, HomeLayout, HomePartialResult, Link, Listing,
	ListingKind, MangaWithChapter, Result,
	alloc::{String, Vec},
	imports::{
		net::{Request, Response},
		std::send_partial_result,
	},
	prelude::*,
};

#[derive(Clone, Copy)]
enum Layout {
	Featured,
	Ranked,
	Chapters,
	Row,
}

/// Home sections in the site's order: listing id, title and layout.
const SECTIONS: &[(&str, &str, Layout)] = &[
	("views", "Most Read", Layout::Featured),
	("trending", "Trending", Layout::Ranked),
	("seasonal", "This Season", Layout::Row),
	("gems", "Hidden Gems", Layout::Row),
	("latest", "Latest Updates", Layout::Chapters),
	("new", "Recently Added", Layout::Row),
	("completed", "Completed", Layout::Row),
];

pub fn section_request(id: &str, page: i32) -> Result<Request> {
	match id {
		"views" => ranking_request(id, &BrowseParams::new("latest", page)),
		"trending" | "gems" => discovery_request(id, &BrowseParams::new("latest", page)),
		"latest" => latest_request(page),
		"seasonal" | "completed" => {
			let mut params = BrowseParams::new("latest", page);
			params.discovery = id.into();
			browse_request(&params)
		}
		"new" => browse_request(&BrowseParams::new(id, page)),
		_ => bail!("Unknown listing"),
	}
}

fn listing(id: &str, name: &str) -> Option<Listing> {
	Some(Listing {
		id: id.into(),
		name: name.into(),
		kind: ListingKind::Default,
	})
}

fn links(response: Response, ratings: &[String]) -> Result<Vec<Link>> {
	Ok(api::series_page(response, ratings, Series::into_manga)?
		.entries
		.into_iter()
		.map(Link::from)
		.collect())
}

/// The feed lists every new chapter, where the site shows each title once, at its newest.
fn latest_entries(response: Response, ratings: &[String]) -> Result<Vec<MangaWithChapter>> {
	let mut entries: Vec<MangaWithChapter> = Vec::new();
	for item in api::parse::<LatestList>(response)?
		.items
		.unwrap_or_default()
	{
		if !allows(&item.series, ratings) {
			continue;
		}
		let Some(manga) = item.series.into_manga() else {
			continue;
		};
		if entries.iter().any(|entry| entry.manga.key == manga.key) {
			continue;
		}
		entries.push(MangaWithChapter {
			manga,
			chapter: item
				.chapter
				.release
				.into_chapter(item.chapter.n, item.chapter.vol.as_ref()),
		});
	}
	Ok(entries)
}

fn fill(
	layout: Layout,
	id: &str,
	name: &str,
	response: Response,
	ratings: &[String],
) -> Result<HomeComponentValue> {
	Ok(match layout {
		Layout::Featured => HomeComponentValue::BigScroller {
			entries: api::series_page(response, ratings, Series::into_full_manga)?.entries,
			auto_scroll_interval: None,
		},
		Layout::Ranked => HomeComponentValue::MangaList {
			ranking: true,
			page_size: Some(5),
			entries: links(response, ratings)?,
			listing: listing(id, name),
		},
		Layout::Chapters => HomeComponentValue::MangaChapterList {
			page_size: Some(5),
			entries: latest_entries(response, ratings)?,
			listing: None,
		},
		Layout::Row => HomeComponentValue::Scroller {
			entries: links(response, ratings)?,
			listing: listing(id, name),
		},
	})
}

impl Home for Toonkotsu {
	fn get_home(&self) -> Result<HomeLayout> {
		let shown = settings::home_sections();
		let sections: Vec<_> = SECTIONS
			.iter()
			.filter(|(id, ..)| {
				shown
					.as_ref()
					.is_none_or(|shown| shown.iter().any(|value| value == id))
			})
			.collect();
		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: sections
				.iter()
				.map(|(_, name, layout)| HomeComponent {
					title: Some(String::from(*name)),
					subtitle: None,
					value: match layout {
						Layout::Featured => HomeComponentValue::empty_big_scroller(),
						Layout::Ranked => HomeComponentValue::empty_manga_list(),
						Layout::Chapters => HomeComponentValue::empty_manga_chapter_list(),
						Layout::Row => HomeComponentValue::empty_scroller(),
					},
				})
				.collect(),
		}));

		let ratings = settings::content_ratings();
		let requests = sections
			.iter()
			.map(|(id, ..)| section_request(id, 1))
			.collect::<Result<Vec<_>>>()?;
		for ((id, name, layout), response) in sections.into_iter().zip(Request::send_all(requests))
		{
			let Ok(value) = response
				.map_err(Into::into)
				.and_then(|response| fill(*layout, id, name, response, &ratings))
			else {
				continue;
			};
			send_partial_result(&HomePartialResult::Component(HomeComponent {
				title: Some(String::from(*name)),
				subtitle: None,
				value,
			}));
		}

		Ok(HomeLayout::default())
	}
}
