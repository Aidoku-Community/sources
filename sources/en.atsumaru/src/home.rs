use crate::{
	Atsumaru,
	api::{FEATURED_FIELDS, FEED_PAGE_SIZE, RANKED, SearchParams, get, lookup_url, parse, request},
	helpers::{feed_allows, spotlight_genre, take_document},
	models::{FeedItem, FeedResponse, SearchResponse},
	settings,
};
use aidoku::{
	Home, HomeComponent, HomeComponentValue, HomeLayout, HomePartialResult, Listing, ListingKind,
	MangaPageResult, Result,
	alloc::{String, Vec, format},
	imports::{
		net::{Request, Response},
		std::send_partial_result,
	},
	prelude::*,
};

const POPULAR: &str = "popular";
const NOVELS: &str = "novels";
const SPOTLIGHT: &str = "genreSpotlight";

const SECTIONS: [(&str, &str); 14] = [
	(POPULAR, "Popular This Week"),
	("hotUpdates", "Hot Updates"),
	("recentlyUpdated", "Recently Updated"),
	("rising", "Rising"),
	("topRated", "Top Rated"),
	(NOVELS, "Most Popular Novels"),
	(SPOTLIGHT, "Spotlight"),
	("hotArrivals", "Hot Arrivals"),
	("mostBookmarked", "Most Bookmarked This Week"),
	("mostTalkedAbout", "Most Talked About This Week"),
	("recentlyAdded", "Recently Added"),
	("hiddenGems", "Hidden Gems"),
	("bingeWorthy", "Binge-Worthy"),
	("mostPolarizing", "Most Polarizing"),
];

fn section_request(id: &str, page: i32) -> Result<Option<Request>> {
	if id == NOVELS {
		let search = SearchParams::novels();
		if search.matches_nothing() {
			return Ok(None);
		}
		return request(&search.url(page, &[])).map(Some);
	}
	let params = SearchParams::new(None);
	if params.matches_nothing() {
		return Ok(None);
	}
	let timeframe = RANKED.contains(&id).then(|| settings::timeframe(id));
	request(&params.feed_url(id, page, timeframe.as_deref())?).map(Some)
}

fn feed_items(response: Response) -> Result<(Vec<FeedItem>, bool)> {
	let items = parse::<FeedResponse>(response)?.items;
	// Feeds give no total, so a full page is the only sign of another.
	let has_next_page = items.len() as i32 == FEED_PAGE_SIZE;
	let types = settings::content_types();
	let items = items
		.into_iter()
		.filter(|item| feed_allows(item.kind.as_deref(), item.medium.as_deref(), &types))
		.collect();
	Ok((items, has_next_page))
}

pub fn listing_page(id: &str, page: i32) -> Result<MangaPageResult> {
	let label_novels = settings::label_novels();
	if id == NOVELS {
		return SearchParams::novels().page(page, label_novels);
	}
	let Some(request) = section_request(id, page)? else {
		return Ok(MangaPageResult::default());
	};
	let (items, has_next_page) = feed_items(request.send()?)?;
	Ok(MangaPageResult {
		entries: items
			.into_iter()
			.map(|item| item.into_manga(label_novels))
			.collect(),
		has_next_page,
	})
}

fn fill(id: &str, name: &str, response: Response) -> Result<HomeComponentValue> {
	let label_novels = settings::label_novels();
	let listing = Some(Listing {
		id: id.into(),
		name: name.into(),
		kind: ListingKind::Default,
	});
	if id == NOVELS {
		let page = parse::<SearchResponse>(response)?.into_page(label_novels);
		return Ok(HomeComponentValue::Scroller {
			entries: page.entries.into_iter().map(Into::into).collect(),
			listing,
		});
	}
	let (items, _) = feed_items(response)?;
	Ok(if id == POPULAR {
		// The feed has no descriptions or authors, so one search request fills them in.
		let ids = items
			.iter()
			.map(|item| item.id.as_str())
			.collect::<Vec<_>>();
		let mut documents = get::<SearchResponse>(&lookup_url(&ids, FEATURED_FIELDS))
			.map(|response| {
				response
					.hits
					.into_iter()
					.map(|hit| hit.document)
					.collect::<Vec<_>>()
			})
			.unwrap_or_default();
		HomeComponentValue::BigScroller {
			entries: items
				.into_iter()
				.map(|item| {
					let document = take_document(&mut documents, &item.id);
					item.into_featured(label_novels, document)
				})
				.collect(),
			auto_scroll_interval: Some(8.0),
		}
	} else {
		HomeComponentValue::Scroller {
			entries: items
				.into_iter()
				.map(|item| item.into_manga(label_novels).into())
				.collect(),
			listing,
		}
	})
}

impl Home for Atsumaru {
	fn get_home(&self) -> Result<HomeLayout> {
		let shown = settings::home_sections();
		let mut sections = Vec::new();
		let mut requests = Vec::new();
		for (id, name) in SECTIONS {
			if shown
				.as_ref()
				.is_some_and(|shown| !shown.iter().any(|value| value == id))
			{
				continue;
			}
			if let Some(request) = section_request(id, 1)? {
				let name: String = if id == SPOTLIGHT {
					format!("{name}: {}", spotlight_genre())
				} else if RANKED.contains(&id) {
					let period = settings::timeframe(id);
					let period = match period.as_str() {
						"daily" => "Today",
						"monthly" => "This Month",
						"all" => "All Time",
						_ => "This Week",
					};
					format!("{} {period}", name.trim_end_matches(" This Week"))
				} else {
					name.into()
				};
				sections.push((id, name));
				requests.push(request);
			}
		}

		// The layout goes out first so the page shows while the sections load.
		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: sections
				.iter()
				.map(|(id, name)| HomeComponent {
					title: Some(name.clone()),
					subtitle: None,
					value: if *id == POPULAR {
						HomeComponentValue::empty_big_scroller()
					} else {
						HomeComponentValue::empty_scroller()
					},
				})
				.collect(),
		}));

		let requested = sections.len();
		// Returned, not sent as partials: the app would keep an empty row for a failed section.
		let components = sections
			.into_iter()
			.zip(Request::send_all(requests))
			.filter_map(|((id, name), response)| {
				let value = response
					.map_err(Into::into)
					.and_then(|response| fill(id, &name, response))
					.ok()?;
				Some(HomeComponent {
					title: Some(name),
					subtitle: None,
					value,
				})
			})
			.collect::<Vec<_>>();
		if requested > 0 && components.is_empty() {
			bail!("Atsumaru didn't answer");
		}
		Ok(HomeLayout { components })
	}
}
