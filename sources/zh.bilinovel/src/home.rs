//! Home page: a skeleton layout is sent first, then the site home page and
//! the ranking page are fetched in parallel and each parsed section fills
//! its slot as a partial result (same pattern as multi.ehentai).

use crate::Bilinovel;
use crate::html::{extract_id, parse_book_item};
use crate::net::Url;
use aidoku::{
	Home, HomeComponent, HomeComponentValue, HomeLayout, HomePartialResult, Link, LinkValue,
	Listing, Manga, Result,
	alloc::{String, Vec, format, vec},
	imports::html::Document,
	imports::net::Request,
	imports::std::send_partial_result,
};

const HERO_TITLE: &str = "勇者新推";
const NEW_TITLE: &str = "新书抢鲜";
const COMPLETED_TITLE: &str = "人气完本";
const RANKING_TITLE: &str = "排行榜";
/// Number of entries shown in the home ranking list.
const MAX_RANKING_ENTRIES: usize = 20;

/// An empty component per home slot, in display order. Partial results
/// replace them one by one as the parallel responses arrive.
fn skeleton() -> Vec<HomeComponent> {
	vec![
		HomeComponent {
			title: None,
			subtitle: None,
			value: HomeComponentValue::ImageScroller {
				links: Vec::new(),
				auto_scroll_interval: Some(5.0),
				width: None,
				height: None,
			},
		},
		HomeComponent {
			title: Some(HERO_TITLE.into()),
			subtitle: None,
			value: HomeComponentValue::empty_scroller(),
		},
		HomeComponent {
			title: Some(NEW_TITLE.into()),
			subtitle: None,
			value: HomeComponentValue::empty_scroller(),
		},
		HomeComponent {
			title: Some(COMPLETED_TITLE.into()),
			subtitle: None,
			value: HomeComponentValue::empty_scroller(),
		},
		HomeComponent {
			title: Some(RANKING_TITLE.into()),
			subtitle: None,
			value: HomeComponentValue::empty_manga_list(),
		},
	]
}

impl Home for Bilinovel {
	fn get_home(&self) -> Result<HomeLayout> {
		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: skeleton(),
		}));

		// The home page supplies the banner and section scrollers; the
		// ranking page supplies the ranked bottom list.
		let home_request = Url::home().request()?;
		let ranking_request = Url::top("monthvisit", 1).request()?;
		let responses: [core::result::Result<aidoku::imports::net::Response, _>; 2] =
			Request::send_all([home_request, ranking_request])
				.try_into()
				.expect("2 requests");
		let [home_response, ranking_response] = responses;

		if let Ok(document) = home_response.and_then(|response| response.get_html()) {
			if let Some(component) = banner_component(&document) {
				send_partial_result(&HomePartialResult::Component(component));
			}
			if let Some(component) = hero_scroller(&document) {
				send_partial_result(&HomePartialResult::Component(component));
			}
			if let Some(component) = book_scroller(&document, NEW_TITLE, "new", "最新入库") {
				send_partial_result(&HomePartialResult::Component(component));
			}
			if let Some(component) =
				book_scroller(&document, COMPLETED_TITLE, "completed", "人气完本")
			{
				send_partial_result(&HomePartialResult::Component(component));
			}
		}

		if let Ok(document) = ranking_response.and_then(|response| response.get_html())
			&& let Some(component) = ranking_component(&document)
		{
			send_partial_result(&HomePartialResult::Component(component));
		}

		Ok(HomeLayout {
			components: Vec::new(),
		})
	}
}

/// Banner carousel at the top: wide cover images.
fn banner_component(document: &Document) -> Option<HomeComponent> {
	let links: Vec<Link> = document
		.select("#slideUl .slide-li a.slide-a")
		.map(|anchors| {
			anchors
				.filter_map(|anchor| {
					let img = anchor.select_first("img.slide-img")?;
					Some(Link {
						title: img.attr("alt").unwrap_or_default(),
						subtitle: None,
						image_url: img.attr("src"),
						value: anchor
							.attr("href")
							.map(|href| LinkValue::Url(absolute_url(&href))),
					})
				})
				.collect()
		})
		.unwrap_or_default();
	if links.is_empty() {
		return None;
	}
	Some(HomeComponent {
		title: None,
		subtitle: None,
		value: HomeComponentValue::ImageScroller {
			links,
			auto_scroll_interval: Some(5.0),
			width: None,
			height: None,
		},
	})
}

/// The `勇者新推` section: a small cover scroller of recommended works.
fn hero_scroller(document: &Document) -> Option<HomeComponent> {
	let module = section(document, HERO_TITLE)?;
	let entries: Vec<Link> = module
		.select(".module-slide-ol a.module-slide-a")
		.map(|anchors| {
			anchors
				.filter_map(|anchor| {
					let img = anchor.select_first("img")?;
					let title = anchor
						.select_first(".module-slide-caption")
						.and_then(|el| el.text())
						.or_else(|| img.attr("alt"))?;
					let href = anchor.attr("href")?;
					Some(Link {
						title,
						subtitle: None,
						image_url: img.attr("data-src").or_else(|| img.attr("src")),
						value: Some(LinkValue::Manga(Manga {
							key: extract_id(&href),
							..Default::default()
						})),
					})
				})
				.collect()
		})
		.unwrap_or_default();
	if entries.is_empty() {
		return None;
	}
	Some(HomeComponent {
		title: Some(HERO_TITLE.into()),
		subtitle: None,
		value: HomeComponentValue::Scroller {
			entries,
			listing: None,
		},
	})
}

/// A book-list home section (`新书抢鲜`/`人气完本`) as a cover scroller,
/// with its "view all" listing attached.
fn book_scroller(
	document: &Document,
	section_title: &str,
	listing_id: &str,
	listing_name: &str,
) -> Option<HomeComponent> {
	let module = section(document, section_title)?;
	let entries: Vec<Link> = module
		.select("ol.book-ol li.book-li a.book-layout")
		.map(|anchors| {
			anchors
				.filter_map(|anchor| parse_book_item(&anchor))
				.map(manga_link)
				.collect()
		})
		.unwrap_or_default();
	if entries.is_empty() {
		return None;
	}
	Some(HomeComponent {
		title: Some(section_title.into()),
		subtitle: None,
		value: HomeComponentValue::Scroller {
			entries,
			listing: Some(listing(listing_id, listing_name)),
		},
	})
}

/// The bottom ranked list, parsed from a `/top/{order}/1.html` page. The
/// document order is the site rank order.
fn ranking_component(document: &Document) -> Option<HomeComponent> {
	let mut entries: Vec<Link> = document
		.select("#list_content li.book-li a.book-layout")
		.map(|anchors| {
			anchors
				.filter_map(|anchor| parse_book_item(&anchor))
				.map(manga_link)
				.collect()
		})
		.unwrap_or_default();
	if entries.is_empty() {
		// Ranking pages without the list container fall back to the
		// generic book list.
		entries = document
			.select("li.book-li a.book-layout")
			.map(|anchors| {
				anchors
					.filter_map(|anchor| parse_book_item(&anchor))
					.map(manga_link)
					.collect()
			})
			.unwrap_or_default();
	}
	if entries.is_empty() {
		return None;
	}
	entries.truncate(MAX_RANKING_ENTRIES);
	Some(HomeComponent {
		title: Some(RANKING_TITLE.into()),
		subtitle: None,
		value: HomeComponentValue::MangaList {
			ranking: true,
			page_size: Some(5),
			entries,
			listing: Some(listing("popular", "排行榜")),
		},
	})
}

/// Find a home page section `.module` by its title.
fn section(document: &Document, title: &str) -> Option<aidoku::imports::html::Element> {
	document.select(".module").and_then(|modules| {
		modules.into_iter().find(|module| {
			module
				.select_first(".module-title")
				.and_then(|el| el.text())
				.is_some_and(|text| text == title)
		})
	})
}

/// Wrap a series entry as a home link; tapping it opens the series by key.
fn manga_link(manga: Manga) -> Link {
	let Manga {
		key, title, cover, ..
	} = manga;
	Link {
		title,
		subtitle: None,
		image_url: cover,
		value: Some(LinkValue::Manga(Manga {
			key,
			..Default::default()
		})),
	}
}

fn listing(id: &str, name: &str) -> Listing {
	Listing {
		id: id.into(),
		name: name.into(),
		..Default::default()
	}
}

fn absolute_url(href: &str) -> String {
	if href.starts_with("http") {
		href.into()
	} else {
		format!("{}{}", crate::BASE_URL, href)
	}
}
