//! Home page: a skeleton layout is sent first, then the three listing pages
//! (排行榜 / 完结小说 / 最新更新) are fetched in parallel; each parsed
//! section fills its slot as a partial result as it arrives.

use crate::Suduguu;
use crate::html::MangaPage as _;
use crate::net::Url;
use aidoku::{
	Home, HomeComponent, HomeComponentValue, HomeLayout, HomePartialResult, Link, LinkValue,
	Listing, Manga, Result,
	alloc::{Vec, vec},
	imports::net::Request,
	imports::std::send_partial_result,
};

const RANKING_TITLE: &str = "排行榜";
const COMPLETED_TITLE: &str = "完结小说";
const LATEST_TITLE: &str = "最新更新";

/// A home section backed by one listing page request.
struct Section {
	category: &'static str,
	title: &'static str,
	listing_id: &'static str,
}

fn sections() -> [Section; 3] {
	[
		Section {
			category: "paihang",
			title: RANKING_TITLE,
			listing_id: "paihang",
		},
		Section {
			category: "wanjie",
			title: COMPLETED_TITLE,
			listing_id: "wanjie",
		},
		Section {
			category: "zuixin",
			title: LATEST_TITLE,
			listing_id: "zuixin",
		},
	]
}

fn skeleton() -> Vec<HomeComponent> {
	vec![
		HomeComponent {
			title: Some(RANKING_TITLE.into()),
			subtitle: None,
			value: HomeComponentValue::empty_scroller(),
		},
		HomeComponent {
			title: Some(COMPLETED_TITLE.into()),
			subtitle: None,
			value: HomeComponentValue::empty_scroller(),
		},
		HomeComponent {
			title: Some(LATEST_TITLE.into()),
			subtitle: None,
			value: HomeComponentValue::empty_scroller(),
		},
	]
}

/// Wrap a series entry as a home link; tapping it opens the series by key.
fn manga_link(manga: Manga) -> Link {
	Link {
		title: manga.title,
		subtitle: None,
		image_url: manga.cover,
		value: Some(LinkValue::Manga(Manga {
			key: manga.key,
			..Default::default()
		})),
	}
}

impl Home for Suduguu {
	fn get_home(&self) -> Result<HomeLayout> {
		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: skeleton(),
		}));

		let sections = sections();
		let requests: Vec<Request> = sections
			.iter()
			.map(|section| {
				Url::List {
					category: section.category.into(),
					page: 1,
				}
				.request()
			})
			.collect::<Result<Vec<_>>>()?;
		let responses = Request::send_all(requests);

		for (section, response) in sections.iter().zip(responses) {
			let Ok(response) = response else { continue };
			let Ok(document) = response.get_html() else {
				continue;
			};
			let Ok(result) = document.manga_page_result(1) else {
				continue;
			};
			let entries = result.entries;

			let scroller_entries: Vec<Link> = entries.into_iter().map(manga_link).collect();
			if !scroller_entries.is_empty() {
				send_partial_result(&HomePartialResult::Component(HomeComponent {
					title: Some(section.title.into()),
					subtitle: None,
					value: HomeComponentValue::Scroller {
						entries: scroller_entries,
						listing: Some(Listing {
							id: section.listing_id.into(),
							name: section.title.into(),
							..Default::default()
						}),
					},
				}));
			}
		}

		Ok(HomeLayout {
			components: Vec::new(),
		})
	}
}
