use crate::{
	graphql::*,
	helpers::{chapter_url, date_from_parts, has_next_page, season_and_year},
	models::*,
	settings,
};
use aidoku::{
	Chapter, HomeComponent, HomeComponentValue, HomeLayout, Link, Listing, Manga, MangaPageResult,
	MangaWithChapter, Result,
	alloc::{String, Vec, vec},
	imports::{
		net::{Request, Response},
		std::current_date,
	},
	prelude::*,
};
use serde_json::{Value, json};

// The home rows that also open as listings, keyed by listing id
#[derive(Clone, Copy, PartialEq)]
pub enum Feed {
	Latest,
	Year,
	Trending,
	Saved,
	Boosted,
	Adaptations,
}

const FEEDS: [Feed; 6] = [
	Feed::Latest,
	Feed::Year,
	Feed::Trending,
	Feed::Saved,
	Feed::Boosted,
	Feed::Adaptations,
];

impl Feed {
	pub fn from_id(id: &str) -> Option<Self> {
		FEEDS.into_iter().find(|feed| feed.id() == id)
	}

	fn id(self) -> &'static str {
		match self {
			Self::Latest => "latest",
			Self::Year => "manga_year",
			Self::Trending => "trending",
			Self::Saved => "most_saved",
			Self::Boosted => "most_boosted",
			Self::Adaptations => "adaptations",
		}
	}

	// Titles follow the site's tabs, whose year and season are the current ones
	fn title(self, now: i64) -> String {
		let (season, year) = season_and_year(now);
		match self {
			Self::Latest => "Latest Updates".into(),
			Self::Year => format!("Manga {year}"),
			Self::Trending => "Trending".into(),
			Self::Saved => "Most Saved".into(),
			Self::Boosted => "Most Boosted".into(),
			Self::Adaptations => format!("{season} {year} Anime"),
		}
	}

	fn listing(self, now: i64) -> Listing {
		Listing {
			id: self.id().into(),
			name: self.title(now),
			..Default::default()
		}
	}

	pub fn request(self, page: i32, now: i64) -> Result<Request> {
		let allow_adult = settings::show_adult();
		match self {
			Self::Trending => request(
				TRENDING_QUERY,
				json!({ "pageSearch": {
					"type": "manga",
					"allowSameShow": false,
					"page": page,
					"size": PAGE_SIZE,
					"allowAdult": allow_adult,
					"allowUnknown": false,
				}}),
			),
			Self::Adaptations => request(
				ADAPTATIONS_QUERY,
				json!({
					"page": page,
					"limit": PAGE_SIZE,
					"translationType": "sub",
					"allowAdult": allow_adult,
					"allowUnknown": false,
					"denyEcchi": false,
				}),
			),
			Self::Latest => browse_request(json!({ "sortBy": "Latest_Update" }), page, "ALL"),
			Self::Year => browse_request(
				json!({ "year": season_and_year(now).1, "sortBy": "Top" }),
				page,
				"ALL",
			),
			Self::Saved => browse_request(json!({ "siteRankSlot": "bookmarked" }), page, "ALL"),
			Self::Boosted => {
				browse_request(json!({ "siteRankSlot": "boostedMonthly" }), page, "ALL")
			}
		}
	}

	pub fn page(self, response: Response, page: i32) -> Result<(Vec<MangaCard>, bool)> {
		Ok(match self {
			Self::Trending => {
				let data: TrendingData = parse(response)?;
				let feed = data.query_latest_page_status;
				// The trending feed sends no total, so a full page is the only sign of another
				let full = feed.recommendations.len() as i32 == PAGE_SIZE;
				let cards = feed
					.recommendations
					.into_iter()
					.filter_map(|recommendation| recommendation.any_card)
					.collect();
				(
					cards,
					feed.total.map_or(full, |total| page * PAGE_SIZE < total),
				)
			}
			Self::Adaptations => {
				let data: AdaptationsData = parse(response)?;
				let feed = data.season_adaptation_mangas;
				(feed.edges, has_next_page(page, feed.page_info.total))
			}
			_ => {
				let data: SearchData = parse(response)?;
				let feed = data.mangas;
				(feed.edges, has_next_page(page, feed.page_info.total))
			}
		})
	}

	pub fn listing_page(self, page: i32) -> Result<MangaPageResult> {
		let (cards, has_next_page) =
			self.page(self.request(page, current_date())?.send()?, page)?;
		Ok(MangaPageResult {
			entries: cards
				.into_iter()
				.filter_map(|card| Manga::try_from(card).ok())
				.collect(),
			has_next_page,
		})
	}
}

// The site's browse query, which search, the latest feed and the ranked tabs share
pub fn browse_request(mut search: Value, page: i32, country: &str) -> Result<Request> {
	if let Some(object) = search.as_object_mut() {
		object.insert("isManga".into(), true.into());
		object.insert("allowAdult".into(), settings::show_adult().into());
		object.insert("allowUnknown".into(), false.into());
		object.insert("denyEcchi".into(), false.into());
	}
	request(
		MANGAS_QUERY,
		json!({
			"search": search,
			"limit": PAGE_SIZE,
			"page": page,
			"translationType": "sub",
			"countryOrigin": country,
		}),
	)
}

// Rows below the popular hero, in the owner's order; `None` is the random rail
const ROWS: [Option<Feed>; 7] = [
	Some(Feed::Year),
	Some(Feed::Trending),
	None,
	Some(Feed::Latest),
	Some(Feed::Saved),
	Some(Feed::Boosted),
	Some(Feed::Adaptations),
];

pub struct HomeData {
	pub popular: Option<Vec<MangaCard>>,
	pub random: Option<Vec<MangaCard>>,
	pub feeds: Vec<(Feed, Vec<MangaCard>)>,
	pub now: i64,
}

pub fn get_home() -> Result<HomeLayout> {
	let now = current_date();
	let allow_adult = settings::show_adult();
	let mut requests = vec![
		request(
			POPULAR_QUERY,
			json!({
				"type": "manga",
				"size": PAGE_SIZE,
				"dateRange": 7,
				"page": 1,
				"allowAdult": allow_adult,
				"allowUnknown": false,
			}),
		)?,
		request(
			RANDOM_QUERY,
			json!({ "format": "manga", "allowAdult": allow_adult, "denyEcchi": false, "slot": 0 }),
		)?,
	];
	let feeds: Vec<Feed> = ROWS.into_iter().flatten().collect();
	for feed in &feeds {
		requests.push(feed.request(1, now)?);
	}
	let mut responses = Request::send_all(requests).into_iter();
	let mut next =
		|| -> Result<Response> { Ok(responses.next().ok_or(error!("Missing home response"))??) };
	// A row that fails drops out; its error is reported only when nothing loads
	let mut last_error = None;
	let mut loaded =
		|result: Result<Vec<MangaCard>>| result.map_err(|error| last_error = Some(error)).ok();
	let popular = loaded(next().and_then(parse::<PopularData>).map(|data| {
		data.query_popular
			.recommendations
			.into_iter()
			.filter_map(|recommendation| recommendation.any_card)
			.collect()
	}));
	let random = loaded(
		next()
			.and_then(parse::<RandomData>)
			.map(|data| data.query_random_recommendation.unwrap_or_default()),
	);
	let mut rows = Vec::new();
	for feed in feeds {
		if let Some(cards) =
			loaded(next().and_then(|response| feed.page(response, 1).map(|(cards, _)| cards)))
		{
			rows.push((feed, cards));
		}
	}
	let layout = layout(HomeData {
		popular,
		random,
		feeds: rows,
		now,
	});
	if layout.components.is_empty() {
		return Err(last_error.unwrap_or(error!("No home content found")));
	}
	Ok(layout)
}

pub fn layout(data: HomeData) -> HomeLayout {
	let HomeData {
		popular,
		random,
		mut feeds,
		now,
	} = data;
	let mut components = Vec::new();
	if let Some(entries) = popular
		.map(|cards| {
			cards
				.into_iter()
				.filter_map(|card| card.into_featured().ok())
				.collect::<Vec<_>>()
		})
		.filter(|entries| !entries.is_empty())
	{
		components.push(HomeComponent {
			title: Some("Popular Weekly".into()),
			value: HomeComponentValue::BigScroller {
				entries,
				auto_scroll_interval: Some(8.0),
			},
			..Default::default()
		});
	}
	let mut random = random;
	for row in ROWS {
		let component = match row {
			None => random
				.take()
				.map(links)
				.filter(|entries| !entries.is_empty())
				.map(|entries| HomeComponent {
					title: Some("Random".into()),
					value: HomeComponentValue::Scroller {
						entries,
						listing: None,
					},
					..Default::default()
				}),
			Some(feed) => feeds
				.iter()
				.position(|(loaded, _)| *loaded == feed)
				.map(|index| feeds.remove(index).1)
				.and_then(|cards| feed_component(feed, cards, now)),
		};
		components.extend(component);
	}
	HomeLayout { components }
}

fn feed_component(feed: Feed, cards: Vec<MangaCard>, now: i64) -> Option<HomeComponent> {
	let value = if feed == Feed::Latest {
		let entries: Vec<MangaWithChapter> = cards.into_iter().filter_map(latest_chapter).collect();
		if entries.is_empty() {
			return None;
		}
		HomeComponentValue::MangaChapterList {
			page_size: Some(5),
			entries,
			listing: Some(feed.listing(now)),
		}
	} else {
		let entries = links(cards);
		if entries.is_empty() {
			return None;
		}
		HomeComponentValue::Scroller {
			entries,
			listing: Some(feed.listing(now)),
		}
	};
	Some(HomeComponent {
		title: Some(feed.title(now)),
		value,
		..Default::default()
	})
}

fn links(cards: Vec<MangaCard>) -> Vec<Link> {
	cards
		.into_iter()
		.filter_map(|card| Manga::try_from(card).ok())
		.map(Link::from)
		.collect()
}

fn latest_chapter(mut card: MangaCard) -> Option<MangaWithChapter> {
	let number = card
		.last_chapter_info
		.take()
		.and_then(|info| info.sub)
		.and_then(|chapter| chapter.chapter_string)?;
	let date_uploaded = card
		.last_chapter_date
		.take()
		.and_then(|date| date.sub)
		.and_then(|date| date_from_parts(&date));
	let manga = Manga::try_from(card).ok()?;
	Some(MangaWithChapter {
		chapter: Chapter {
			chapter_number: number.parse().ok(),
			date_uploaded,
			url: Some(chapter_url(&manga.key, &number)),
			key: number,
			..Default::default()
		},
		manga,
	})
}
