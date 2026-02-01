//
// source made by apix <@apix0n>
//

#![no_std]
use aidoku::{
	AlternateCoverProvider, Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home,
	HomeComponent, HomeLayout, Listing, ListingKind, ListingProvider, Manga, MangaPageResult,
	MangaWithChapter, Page, PageContent, Result, Source,
	alloc::{String, Vec, string::ToString, vec},
	imports::{net::Request, std::send_partial_result},
	prelude::*,
};

mod models;
use crate::models::{ChapterData, ChapterEndpointData, Series, SeriesList, map_bigsolo_status};

const BASE_URL: &str = "https://bigsolo.org";

struct BigSolo;

impl Source for BigSolo {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		_page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let series_list: SeriesList =
			Request::get(format!("{BASE_URL}/data/series"))?.json_owned::<SeriesList>()?;

		let mut entries: Vec<Manga> = Vec::new();

		// helper function for searching
		let matches_query = |series: &Series| -> bool {
			if let Some(ref search_query) = query {
				let title_lower = series.title.to_lowercase();
				let author_lower = series.author.to_lowercase();
				let artist_lower = series.artist.to_lowercase();
				let query_lower = search_query.to_lowercase();

				title_lower.contains(&query_lower)
					|| author_lower.contains(&query_lower)
					|| artist_lower.contains(&query_lower)
					|| series
						.alternative_titles
						.iter()
						.any(|alt_title| alt_title.to_lowercase().contains(&query_lower))
			} else {
				true
			}
		};

		// process series array
		for series in series_list.series {
			if !matches_query(&series) {
				continue;
			}

			entries.push(series.into());
		}

		// process os array and add One-shot tag
		for series in series_list.os {
			if !matches_query(&series) {
				continue;
			}

			let mut manga: Manga = series.into();
			if let Some(tags) = manga.tags.as_mut() {
				tags.push(String::from("One-shot"));
			} else {
				manga.tags = Some(vec![String::from("One-shot")]);
			}

			entries.push(manga);
		}

		Ok(MangaPageResult {
			entries,
			has_next_page: false,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let api_url = format!("{BASE_URL}/data/series/{}", manga.key);
		let series_data: Series = Request::get(&api_url)?.json_owned::<Series>()?;

		let title = series_data.title.clone();

		if needs_details {
			manga.title = title;
			manga.cover = Some(series_data.cover.url_hq.clone());
			manga.authors = Some(vec![series_data.author.clone()]);
			manga.artists = Some(vec![series_data.artist.clone()]);
			manga.description = Some(series_data.description.clone());
			manga.status = map_bigsolo_status(&series_data.status);
			let mut tags = series_data.tags.clone();
			if series_data.os.unwrap_or(false) {
				tags.push(String::from("One-shot"));
			}
			manga.tags = Some(tags);
			manga.url = Some(format!("{BASE_URL}/{}", series_data.slug));

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let mut chapter_vec: Vec<(String, models::ChapterData)> =
				series_data.chapters.into_iter().collect();
			use core::cmp::Ordering;
			chapter_vec.sort_by(|(a_key, _), (b_key, _)| {
				let a_num = a_key.parse::<f32>().unwrap_or(0.0);
				let b_num = b_key.parse::<f32>().unwrap_or(0.0);
				b_num.partial_cmp(&a_num).unwrap_or(Ordering::Equal)
			});
			manga.chapters = Some(
				chapter_vec
					.into_iter()
					.map(|(chapter_key, chapter)| {
						let chapter_number = chapter_key.parse().ok();
						let url = format!("{BASE_URL}/{}/{}", series_data.slug, chapter_key);
						Chapter {
							key: chapter_key,
							title: Some(chapter.title),
							chapter_number,
							volume_number: chapter.volume.unwrap_or_default().parse().ok(),
							date_uploaded: Some(chapter.timestamp),
							url: Some(url),
							scanlators: Some(chapter.teams),
							locked: chapter.licensed.unwrap_or(false),
							..Default::default()
						}
					})
					.collect(),
			);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let parsed = Request::get(format!(
			"{BASE_URL}/data/series/{}/{}",
			manga.key, chapter.key
		))?
		.json_owned::<ChapterEndpointData>()?;

		Ok(parsed
			.images
			.into_iter()
			.map(|image| Page {
				content: PageContent::url(image),
				..Default::default()
			})
			.collect())
	}
}

impl Home for BigSolo {
	fn get_home(&self) -> Result<HomeLayout> {
		let all_entries = self.get_search_manga_list(None, 1, Vec::new())?.entries;

		let home_data: SeriesList =
			Request::get(format!("{BASE_URL}/data/series"))?.json_owned::<SeriesList>()?;

		let reco_entries = home_data
			.reco
			.into_iter()
			.map(Into::into)
			.collect::<Vec<Manga>>();

		// get the latest chapters from the home data (series and os)
		let mut latest_chapters: Vec<(String, String, ChapterData)> = home_data
			.series
			.into_iter()
			.chain(home_data.os)
			.flat_map(|series| {
				series
					.chapters
					.into_iter()
					.map(move |(chapter_key, chapter)| (series.slug.clone(), chapter_key, chapter))
			})
			.collect();

		// Sort by timestamp (most recent first) and take the 10 last
		latest_chapters.sort_by(|a, b| b.2.timestamp.cmp(&a.2.timestamp));
		latest_chapters.truncate(10);

		let latest_chapters: Vec<MangaWithChapter> = latest_chapters
			.into_iter()
			.filter_map(|(series_slug, chapter_key, chapter_data)| {
				// Find the manga for this chapter
				let manga = all_entries.iter().find(|m| m.key == series_slug)?.clone();

				let chapter_number = chapter_key.parse().ok().unwrap_or_default();
				let url = format!("{BASE_URL}/{}/{}", series_slug, chapter_key);
				let chapter = Chapter {
					key: chapter_key,
					title: Some(chapter_data.title),
					chapter_number: Some(chapter_number),
					volume_number: chapter_data.volume.unwrap_or_default().parse().ok(),
					date_uploaded: Some(chapter_data.timestamp),
					url: Some(url),
					scanlators: Some(chapter_data.teams),
					locked: chapter_data.licensed.unwrap_or(false),
					..Default::default()
				};

				Some(MangaWithChapter { manga, chapter })
			})
			.collect();

		// filter mangas for the home views (using the 'One-shot' tag)
		let series_entries: Vec<Manga> = all_entries
			.iter()
			.filter(|manga| {
				!manga
					.tags
					.as_ref()
					.unwrap_or(&Vec::new())
					.contains(&String::from("One-shot"))
			})
			.cloned()
			.collect();

		let oneshot_entries: Vec<Manga> = all_entries
			.iter()
			.filter(|manga| {
				manga
					.tags
					.as_ref()
					.unwrap_or(&Vec::new())
					.contains(&String::from("One-shot"))
			})
			.cloned()
			.collect();

		Ok(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some(String::from("Recommandations")),
					subtitle: None,
					value: aidoku::HomeComponentValue::BigScroller {
						entries: reco_entries,
						auto_scroll_interval: Some(7.0),
					},
				},
				HomeComponent {
					title: Some(String::from("Derniers chapitres")),
					subtitle: None,
					value: aidoku::HomeComponentValue::MangaChapterList {
						page_size: None,
						entries: latest_chapters,
						listing: None,
					},
				},
				HomeComponent {
					title: Some(String::from("Séries")),
					subtitle: None,
					value: aidoku::HomeComponentValue::Scroller {
						entries: series_entries.iter().cloned().map(|m| m.into()).collect(),
						listing: None,
					},
				},
				HomeComponent {
					title: Some(String::from("One-shot")),
					subtitle: None,
					value: aidoku::HomeComponentValue::Scroller {
						entries: oneshot_entries.iter().cloned().map(|m| m.into()).collect(),
						listing: None,
					},
				},
			],
		})
	}
}

impl AlternateCoverProvider for BigSolo {
	fn get_alternate_covers(&self, manga: Manga) -> Result<Vec<String>> {
		let api_url = format!("{BASE_URL}/data/series/{}", manga.key);

		let series_data: Series = Request::get(&api_url)?.json_owned::<Series>()?;

		// extract covers from covers_gallery
		let covers: Vec<String> = series_data
			.covers
			.into_iter()
			.map(|cover| cover.url_hq)
			.collect();

		Ok(covers)
	}
}

impl DeepLinkHandler for BigSolo {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		// Strip protocol + domain
		let path = match url.split(BASE_URL).nth(1) {
			Some(p) => p.trim_matches('/'),
			None => return Ok(None),
		};

		let parts: Vec<&str> = path.split('/').collect();

		match parts.as_slice() {
			// https://bigsolo.org/series (catalogue)
			["series"] => Ok(Some(DeepLinkResult::Listing({
				Listing {
					id: "catalogue".to_string(),
					name: "Catalogue".to_string(),
					kind: ListingKind::Default,
				}
			}))),

			// https://bigsolo.org/{series}
			[series] => Ok(Some(DeepLinkResult::Manga {
				key: series.to_string(),
			})),

			// https://bigsolo.org/{series}/{chapter}
			[series, chapter] => Ok(Some(DeepLinkResult::Chapter {
				manga_key: series.to_string(),
				key: chapter.to_string(),
			})),

			_ => Ok(None),
		}
	}
}

impl ListingProvider for BigSolo {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		if listing.id == "catalogue" {
			// we need to sort by title because the catalogue is not sorted by title
			let mut mangas = self.get_search_manga_list(None, page, Vec::new())?.entries;
			mangas.sort_by(|a, b| a.title.cmp(&b.title));
			Ok(MangaPageResult {
				entries: mangas,
				has_next_page: false,
			})
		} else {
			bail!("Unknown listing")
		}
	}
}

register_source!(
	BigSolo,
	Home,
	AlternateCoverProvider,
	ListingProvider,
	DeepLinkHandler
);
