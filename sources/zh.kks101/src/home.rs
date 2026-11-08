//! Home page: a skeleton layout is sent first, then the four listing pages
//! (新書榜 / 人氣榜 / 推薦榜 / 全本小說) are fetched in parallel; each
//! parsed section fills its slot as a partial result as it arrives.

use crate::Kks101;
use crate::html::MangaPage as _;
use crate::net::Url;
use aidoku::{
	Home, HomeComponent, HomeComponentValue, HomeLayout, HomePartialResult, Link, LinkValue,
	Listing, Manga, Result,
	alloc::{String, Vec, vec},
	imports::net::{Request, Response},
	imports::std::send_partial_result,
};

const NEW_TITLE: &str = "新書榜";
const POPULAR_TITLE: &str = "人氣榜";
const RECOMMENDED_TITLE: &str = "推薦榜";
const COMPLETED_TITLE: &str = "全本小說";

/// A home section backed by one `/novels/...` listing page request.
struct Section {
	order: &'static str,
	isfull: &'static str,
	title: &'static str,
	listing_id: &'static str,
	listing_name: &'static str,
}

/// The four sections in display order.
fn sections() -> [Section; 4] {
	[
		Section {
			order: "newhot",
			isfull: "0",
			title: NEW_TITLE,
			listing_id: "new",
			listing_name: NEW_TITLE,
		},
		Section {
			order: "weekvisit",
			isfull: "0",
			title: POPULAR_TITLE,
			listing_id: "popular",
			listing_name: POPULAR_TITLE,
		},
		Section {
			order: "allvote",
			isfull: "0",
			title: RECOMMENDED_TITLE,
			listing_id: "recommended",
			listing_name: RECOMMENDED_TITLE,
		},
		Section {
			order: "newhot",
			isfull: "1",
			title: COMPLETED_TITLE,
			listing_id: "completed",
			listing_name: COMPLETED_TITLE,
		},
	]
}

/// An empty component per home slot, in display order. Partial results
/// replace them one by one as the parallel responses arrive.
fn skeleton() -> Vec<HomeComponent> {
	vec![
		HomeComponent {
			title: Some(NEW_TITLE.into()),
			subtitle: None,
			value: HomeComponentValue::empty_scroller(),
		},
		HomeComponent {
			title: Some(POPULAR_TITLE.into()),
			subtitle: None,
			value: HomeComponentValue::empty_scroller(),
		},
		HomeComponent {
			title: Some(RECOMMENDED_TITLE.into()),
			subtitle: None,
			value: HomeComponentValue::empty_scroller(),
		},
		HomeComponent {
			title: Some(COMPLETED_TITLE.into()),
			subtitle: None,
			value: HomeComponentValue::empty_scroller(),
		},
	]
}

/// Wrap a series entry as a home link; tapping it opens the series by key.
/// Takes ownership so no cloning is needed.
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

impl Home for Kks101 {
	fn get_home(&self) -> Result<HomeLayout> {
		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: skeleton(),
		}));

		let sections = sections();
		let requests: Vec<Request> = sections
			.iter()
			.map(|section| {
				Url::List {
					order: section.order,
					tagid: String::from("0"),
					isfull: String::from(section.isfull),
					page: 1,
				}
				.request()
			})
			.collect::<Result<Vec<_>>>()?;
		let responses: [core::result::Result<Response, _>; 4] =
			Request::send_all(requests).try_into().expect("4 requests");

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
							name: section.listing_name.into(),
							..Default::default()
						}),
					},
				}));
			}
		}

		// All sections were streamed as partial results; nothing left for
		// the final layout.
		Ok(HomeLayout {
			components: Vec::new(),
		})
	}
}
