#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeLayout, ImageRequestProvider,
	Listing, ListingProvider, Manga, MangaPageResult, Page, PageContent, PageContext, Result,
	Source,
	alloc::{String, Vec, vec},
	imports::{
		net::{Request, TimeUnit, set_rate_limit},
		std::{current_date, send_partial_result},
	},
	prelude::*,
};
use core::cell::RefCell;
use serde_json::{Map, Value, json};

mod graphql;
mod helpers;
mod home;
mod models;
mod reader;
mod settings;

use graphql::*;
use helpers::{day_range, has_next_page, parse_chapters, season_and_year};
use home::{Feed, browse_request};
use models::*;

const SORTS: [&str; 6] = [
	"Top",
	"Latest_Update",
	"Release_Year",
	"Name_ASC",
	"Name_DESC",
	"Popular",
];

struct MKissa {
	reader: RefCell<reader::Reader>,
}

impl Source for MKissa {
	fn new() -> Self {
		// The API answers bursts with "Too many requests"
		set_rate_limit(3, 1, TimeUnit::Seconds);
		Self {
			reader: RefCell::new(reader::Reader::default()),
		}
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		// A pasted series link finds that series
		if let Some(id) = query
			.as_deref()
			.and_then(|query| query.strip_prefix(BASE_URL))
			.and_then(|path| path.strip_prefix("/manga/"))
			.and_then(|path| path.split(['/', '?', '#']).next())
			.filter(|id| !id.is_empty())
			&& let Ok(DetailsData { manga: Some(card) }) =
				fetch::<DetailsData>(CARD_QUERY, json!({ "id": id }))
			&& let Ok(manga) = Manga::try_from(card)
		{
			return Ok(MangaPageResult {
				entries: vec![manga],
				has_next_page: false,
			});
		}

		// The app leaves out the default sort, and the API then sorts by latest update
		let mut search = Map::from_iter([("sortBy".into(), SORTS[0].into())]);
		let mut country = String::from("ALL");
		let mut genres: Vec<String> = Vec::new();
		let mut excluded_genres: Vec<String> = Vec::new();
		if let Some(query) = query {
			search.insert("query".into(), query.into());
		}
		for filter in filters {
			match filter {
				FilterValue::Sort { index, .. } => {
					if let Some(sort) = SORTS.get(index as usize) {
						search.insert("sortBy".into(), (*sort).into());
					}
				}
				FilterValue::Select { id, value } => match id.as_str() {
					"country" => country = value,
					// A tapped tag sends its display name; magazines search by name in their own field
					"genre" => match find_tag(&value)? {
						Some(tag) if tag.slug.ends_with("-magazine") => {
							search.insert("magazine".into(), tag.name.into());
						}
						Some(tag) => {
							search.insert("tags".into(), vec![tag.slug].into());
						}
						None => genres.push(value),
					},
					"day" => {
						let (start, end) = value
							.parse()
							.ok()
							.map(|days_ago| day_range(days_ago, current_date()))
							.ok_or(error!("Unknown update day"))?;
						search.insert("dateRangeStart".into(), start.into());
						search.insert("dateRangeEnd".into(), end.into());
					}
					"season" => {
						search.insert("season".into(), value.into());
						// The site's season picker always carries a year, the current one by default
						search
							.entry("year")
							.or_insert_with(|| season_and_year(current_date()).1.into());
					}
					"chapters" => {
						// Ranges read `11-50`; the last, `2001`, has no end
						let (start, end) = value.split_once('-').unwrap_or((&value, ""));
						for (key, bound) in [("epRangeStart", start), ("epRangeEnd", end)] {
							if let Ok(bound) = bound.parse::<i32>() {
								search.insert(key.into(), bound.into());
							}
						}
					}
					_ => {}
				},
				FilterValue::Text { id, value } if id == "year" => {
					let year: i32 = value.trim().parse().map_err(|_| error!("Enter a year"))?;
					search.insert("year".into(), year.into());
				}
				// The site's panel only includes; the API also excludes
				FilterValue::MultiSelect {
					included, excluded, ..
				} => {
					genres.extend(included);
					excluded_genres.extend(excluded);
				}
				_ => {}
			}
		}
		for (field, values) in [("genres", genres), ("excludeGenres", excluded_genres)] {
			if !values.is_empty() {
				search.insert(field.into(), values.into());
			}
		}
		let data: SearchData =
			parse(browse_request(Value::Object(search), page, &country)?.send()?)?;
		Ok(MangaPageResult {
			has_next_page: has_next_page(page, data.mangas.page_info.total),
			entries: data
				.mangas
				.edges
				.into_iter()
				.filter_map(|card| Manga::try_from(card).ok())
				.collect(),
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let data: DetailsData = fetch(DETAILS_QUERY, json!({ "id": manga.key }))?;
			let details = data
				.manga
				.ok_or(error!("Series not found"))?
				.into_details()?;
			manga.copy_from(details);
			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let data: ChaptersData = fetch(
				CHAPTERS_QUERY,
				json!({ "id": manga.key, "showId": format!("manga@{}", manga.key) }),
			)?;
			manga.chapters = Some(parse_chapters(data, &manga.key)?);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		Ok(self
			.reader
			.borrow_mut()
			.page_urls(&manga.key, &chapter.key)?
			.into_iter()
			.map(|url| Page {
				content: PageContent::url(url),
				..Default::default()
			})
			.collect())
	}
}

impl Home for MKissa {
	fn get_home(&self) -> Result<HomeLayout> {
		home::get_home()
	}
}

impl ListingProvider for MKissa {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		Feed::from_id(&listing.id)
			.ok_or(error!("Unknown listing"))?
			.listing_page(page)
	}
}

impl ImageRequestProvider for MKissa {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?
			.header("Referer", &format!("{BASE_URL}/"))
			.header("Origin", BASE_URL))
	}
}

impl DeepLinkHandler for MKissa {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some((_, path)) = url.split_once("/manga/") else {
			return Ok(None);
		};
		let mut segments = path.split(['/', '?', '#']);
		let Some(manga_key) = segments.next().filter(|key| !key.is_empty()) else {
			return Ok(None);
		};
		// Chapter paths read `chapter-12-sub`
		if let Some(number) = segments
			.next()
			.and_then(|segment| segment.strip_prefix("chapter-"))
			.map(|rest| rest.trim_end_matches("-sub"))
			.filter(|number| !number.is_empty())
		{
			return Ok(Some(DeepLinkResult::Chapter {
				manga_key: manga_key.into(),
				key: number.into(),
			}));
		}
		Ok(Some(DeepLinkResult::Manga {
			key: manga_key.into(),
		}))
	}
}

register_source!(
	MKissa,
	Home,
	ListingProvider,
	ImageRequestProvider,
	DeepLinkHandler
);
