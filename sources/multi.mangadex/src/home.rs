use crate::MangaDex;
use crate::{models::*, settings};
use crate::{API_URL, CUSTOM_LISTS};
use aidoku::imports::net::Response;
use aidoku::Link;
use aidoku::{
	alloc::{vec, String, Vec},
	imports::{
		error::AidokuError,
		net::{Request, RequestError},
		std::{current_date, send_partial_result},
	},
	prelude::*,
	Home, HomeComponent, HomeLayout, HomePartialResult, Listing, ListingKind, Manga,
	MangaWithChapter, Result,
};
use chrono::{TimeZone, Utc};
use hashbrown::HashSet;
use regex::Regex;

impl Home for MangaDex {
	fn get_home(&self) -> Result<HomeLayout> {
		// fetch custom list titles and manga ids
		struct CustomList<'a> {
			id: &'a str,
			name: String,
			entries: Vec<&'a str>,
		}
		let mut custom_list_requests = Request::send_all(
			CUSTOM_LISTS
				.iter()
				.map(|list| format!("{API_URL}/list/{list}"))
				.map(|url| Request::get(url).expect("invalid url format")),
		);
		let custom_lists = &mut custom_list_requests
			.iter_mut()
			.filter_map(|req| {
				req.as_mut()
					.ok()?
					.get_json::<DexResponse<DexCustomList>>()
					.ok()
			})
			.map(|res| CustomList {
				id: res.data.id,
				name: res.data.attributes.name,
				entries: res
					.data
					.relationships
					.iter()
					.filter_map(|relationship| {
						if relationship.r#type == "manga" {
							Some(relationship.id)
						} else {
							None
						}
					})
					.collect(),
			})
			.collect::<Vec<_>>();

		// fetch seasonal list
		struct SeasonalList<'a> {
			name: String,
			entries: Vec<&'a str>,
		}
		let owner_user_id = "d2ae45e0-b5e2-4e7f-a688-17925c2d7d6b";
		let mut seasonal_res = Request::get(format!("{API_URL}/user/{owner_user_id}/list"))?.send()?;

		let seasonal_re = Regex::new(r"^Seasonal:\s*(?P<season>Winter|Spring|Summer|Fall)\s*(?P<year>\d{4})$").unwrap();
		let season_to_rank = |season: &str| -> u8 {
			match season.to_lowercase().as_str() {
				"winter" => 1,
				"spring" => 2,
				"summer" => 3,
				"fall" => 4,
				_ => 0,
			}
		};
		
  		let mut current_seasonal_lists = seasonal_res
			.get_json::<DexResponse<Vec<DexCustomList>>>()?
			.data
			.iter()
			.filter_map(|item| {
				let name = &item.attributes.name;
				let captures = seasonal_re.captures(name)?;
				let year = captures.name("year")?.as_str().parse::<u16>().ok();
				let season_rank = season_to_rank(captures.name("season")?.as_str());

				let manga_ids = item.relationships.iter().filter_map(|relationship| {
					if relationship.r#type == "manga" {
						Some(relationship.id)
					} else {
						None
					}
				})
				.collect::<Vec<&str>>();

				Some((year, season_rank, name, manga_ids))
			})
			.max_by(|(y1, s1, _, _), (y2, s2, _, _)| (y1,s1).cmp(&(y2,s2)))
			.map(|(_, _, name, manga_ids)| SeasonalList { name: name.clone(), entries: manga_ids });

		// send basic home layout
		{
			let mut components = vec![
				HomeComponent {
					title: Some("Popular New Titles".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_big_scroller(),
				},
				HomeComponent {
					title: Some("Latest Updates".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_manga_chapter_list(),
				},
			];
			for CustomList { name, .. } in custom_lists.iter() {
				components.push(HomeComponent {
					title: Some(name.clone()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_scroller(),
				});
			}
			for SeasonalList { name, .. } in current_seasonal_lists.iter() {
				components.push(HomeComponent {
					title: Some(name.clone()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_scroller()
				});
			}
			components.push(HomeComponent {
				title: Some("Recently Added".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::empty_scroller(),
			});
			send_partial_result(&HomePartialResult::Layout(HomeLayout { components }));
		}

		let languages = settings::get_languages_with_key("translatedLanguage")?;
		let content_ratings = settings::get_content_ratings()?;

		let responses: [core::result::Result<Response, RequestError>; 3] = Request::send_all([
			// popular
			Request::get(format!(
				"{API_URL}/manga\
					?includes[]=cover_art\
					&includes[]=artist\
					&includes[]=author\
					&order[followedCount]=desc\
					&hasAvailableChapters=true\
					&createdAtSince={}\
					{content_ratings}",
				// gmt time, one month ago
				Utc.timestamp_opt(current_date() as i64 - 2630000, 0)
					.unwrap()
					.format("%Y-%m-%dT%H:%M:%S")
			))?,
			// recently added
			Request::get(format!(
				"{API_URL}/manga\
					?limit=15\
					&order[createdAt]=desc\
					&includes[]=cover_art\
					{content_ratings}"
			))?,
			// latest
			Request::get(format!(
				"{API_URL}/chapter\
					?includes[]=scanlation_group\
					&limit=15\
					&order[readableAt]=desc\
					{languages}\
					{content_ratings}"
			))?,
		])
		.try_into()
		.expect("requests vec length should be 3");

		let [popular_res, recent_res, chapters_res] = responses;

		// popular scroller
		{
			let popular_manga = popular_res?
				.get_json::<DexResponse<Vec<DexManga>>>()
				.map_err(|_| AidokuError::message("Failed to parse popular manga"))?
				.data
				.iter()
				.map(|value| Manga {
					key: String::from(value.id),
					title: value.title().unwrap_or_default(),
					cover: value.cover(),
					description: value.description(),
					tags: Some(value.tags()),
					content_rating: value.content_rating(),
					..Default::default()
				})
				.collect::<Vec<Manga>>();
			send_partial_result(&HomePartialResult::Component(HomeComponent {
				title: Some(String::from("Popular New Titles")),
				subtitle: None,
				value: aidoku::HomeComponentValue::BigScroller {
					entries: popular_manga,
					auto_scroll_interval: Some(10.0),
				},
			}));
		}

		// recently added scroller
		{
			let added_manga = recent_res?
				.get_json::<DexResponse<Vec<DexManga>>>()
				.map_err(|_| AidokuError::message("Failed to parse recent manga"))?
				.data
				.into_iter()
				.map(|value| value.into_basic_manga().into())
				.collect::<Vec<Link>>();

			send_partial_result(&HomePartialResult::Component(HomeComponent {
				title: Some(String::from("Recently Added")),
				subtitle: None,
				value: aidoku::HomeComponentValue::Scroller {
					entries: added_manga,
					listing: Some(Listing {
						id: String::from("recent"),
						name: String::from("Recently Added"),
						kind: ListingKind::Default,
					}),
				},
			}));
		}

		// latest chapters list
		{
			let chapters_data = chapters_res?
				.get_data()
				.map_err(|_| AidokuError::message("Failed to fetch latest chapters"))?;
			// get one chapter per unique manga
			let mut seen = HashSet::new();
			let chapters: Vec<DexChapter> =
				serde_json::from_slice::<DexResponse<Vec<DexChapter>>>(&chapters_data)
					.map_err(|_| AidokuError::JsonParseError)?
					.data
					.into_iter()
					.filter(|chapter| {
						chapter
							.manga_id()
							.map(|id| seen.insert(id))
							.unwrap_or(false)
					})
					.take(6)
					.collect();

			let manga_ids = chapters
				.iter()
				.filter_map(|value| value.manga_id().map(|m| format!("&ids[]={m}")))
				.collect::<String>();

			let latest_manga_url = format!(
				"{API_URL}/manga\
					?includes[]=cover_art\
					{content_ratings}\
					{manga_ids}"
			);
			let latest_manga = Request::get(latest_manga_url)?
				.send()?
				.get_json::<DexResponse<Vec<DexManga>>>()?
				.data
				.into_iter()
				.map(|value| value.into_basic_manga())
				.collect::<Vec<Manga>>();

			let latest_chapters = chapters
				.into_iter()
				.map(|value| MangaWithChapter {
					manga: latest_manga
						.iter()
						.find(|m| m.key == value.manga_id().expect("need manga"))
						.expect("need manga!")
						.clone(),
					chapter: value.into(),
				})
				.collect::<Vec<MangaWithChapter>>();

			send_partial_result(&HomePartialResult::Component(HomeComponent {
				title: Some(String::from("Latest Updates")),
				subtitle: None,
				value: aidoku::HomeComponentValue::MangaChapterList {
					page_size: None,
					entries: latest_chapters,
					listing: Some(Listing {
						id: String::from("latest"),
						name: String::from("Latest Updates"),
						kind: ListingKind::Default,
					}),
				},
			}));
		}

		// custom lists components
		{
			let custom_list_responses = Request::send_all(custom_lists.iter().map(|list| {
				Request::get(format!(
					"{API_URL}/manga\
						?limit=32\
						&includes[]=cover_art\
						{content_ratings}\
						&ids[]={}",
					list.entries.join("&ids[]=")
				))
				.unwrap()
			}));
			let custom_lists = custom_lists
				.iter_mut()
				.zip(custom_list_responses)
				.filter_map(|(list, res)| {
					Some((
						list.id,
						list.name.clone(),
						res.ok()?
							.get_json::<DexResponse<Vec<DexManga>>>()
							.map(|response| {
								response
									.data
									.into_iter()
									.map(|value| value.into_basic_manga().into())
									.collect::<Vec<Link>>()
							})
							.ok()?,
					))
				})
				.collect::<Vec<(&str, String, Vec<Link>)>>();

			for (id, name, entries) in custom_lists {
				send_partial_result(&HomePartialResult::Component(HomeComponent {
					title: Some(name.clone()),
					subtitle: None,
					value: aidoku::HomeComponentValue::Scroller {
						entries,
						listing: Some(Listing {
							id: format!("list-{id}"),
							name,
							kind: ListingKind::Default,
						}),
					},
				}));
			}
		}

		// same from a custom lists
		{
			let seasonal_list_responses = Request::send_all(current_seasonal_lists.iter().map(|list| {
				Request::get(format!(
					"{API_URL}/manga\
						?limit=100\
						&includes[]=cover_art\
						{content_ratings}\
						&ids[]={}",
					list.entries.join("&ids[]=")
				))
				.unwrap()
			}));
			let current_seasonal_lists = current_seasonal_lists
				.iter_mut()
				.zip(seasonal_list_responses)
				.filter_map(|(list, res)| {
					Some((
						list.name.clone(),
						res.ok()?
							.get_json::<DexResponse<Vec<DexManga>>>()
							.map(|response| {
								response
									.data
									.into_iter()
									.map(|value| value.into_basic_manga().into())
									.collect::<Vec<Link>>()
							})
							.ok()?,
					))
				})
				.collect::<Vec<(String, Vec<Link>)>>();

			for (name, entries) in current_seasonal_lists {
				send_partial_result(&HomePartialResult::Component(HomeComponent {
					title: Some(name.clone()),
					subtitle: None,
					value: aidoku::HomeComponentValue::Scroller {
						entries,
						listing: Some(Listing {
							id: String::from("seasonal"),
							name,
							kind: ListingKind::Default,
						}),
					},
				}));
			}
		}

		Ok(HomeLayout::default())
	}
}
