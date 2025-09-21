//
// source made by apix <@apix0n>
//

#![no_std]
use aidoku::{
	AidokuError, AlternateCoverProvider, Chapter, FilterValue, Home, HomeComponent, HomeLayout,
	Manga, MangaPageResult, MangaStatus, Page, PageContent, Result, Source,
	alloc::{String, Vec, vec},
	imports::{net::Request, std::send_partial_result},
	prelude::*,
};

mod models;
use models::ImageList;
mod slugify;
use slugify::slugify;

use crate::models::{ConfigJson, SeriesData};

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
		let manga_list = BigSolo::get_bigsolo_manga_files();

		let mut entries: Vec<Manga> = Vec::new();

		for file_path in manga_list {
			let response = match Request::get(&file_path).ok().and_then(|r| r.string().ok()) {
				Some(r) => r,
				None => {
					continue; // Skip files that can't be loaded
				}
			};

			let series_data: models::SeriesData =
				match serde_json::from_str::<SeriesData>(&response) {
					Ok(data) => data,
					Err(_) => {
						continue; // Skip files that can't be parsed
					}
				};

			// Filter by query if provided
			if let Some(ref search_query) = query {
				let title_lower = series_data.title.to_lowercase();
				let author_lower = series_data.author.to_lowercase();
				let artist_lower = series_data.artist.to_lowercase();
				let query_lower = search_query.to_lowercase();

				if !title_lower.contains(&query_lower)
					&& !author_lower.contains(&query_lower)
					&& !artist_lower.contains(&query_lower)
				{
					continue; // Skip if no match
				}
			}

			// convert SeriesData to a minimal Manga entry (to load full data into `get_manga_update`)
			let title = series_data.title.clone();
			let slug = slugify(&title);

			// extract filename from file_path to use as manga entry key
			let file_name = file_path
				.split('/')
				.next_back()
				.unwrap_or("unknown")
				.replace(".json", "");

			// add 'One-shot' tag if os is true (for homepage filtering)
			let mut tags = series_data.tags;
			if series_data.os.unwrap_or(false) {
				tags.push(String::from("One-shot"));
			}

			let manga = Manga {
				key: file_name.clone(),
				title,
				cover: Some(series_data.cover_low),
				authors: Some(vec![series_data.author]),
				artists: Some(vec![series_data.artist]),
				description: Some(series_data.description),
				status: BigSolo::map_bigsolo_status(&series_data.release_status),
				tags: Some(tags),
				url: Some(format!("https://bigsolo.org/{slug}")),
				..Default::default()
			};

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
		let file_path = format!("https://bigsolo.org/data/series/{}.json", manga.key);
		let response = Request::get(&file_path)?.string()?;
		let series_data: SeriesData =
			serde_json::from_str(&response).map_err(AidokuError::message)?;

		let title = series_data.title.clone();
		let slug = slugify(&title);

		if needs_details {
			manga.title = title;
			manga.cover = Some(series_data.cover_hq.clone());
			manga.authors = Some(vec![series_data.author.clone()]);
			manga.artists = Some(vec![series_data.artist.clone()]);
			manga.description = Some(series_data.description.clone());
			manga.status = BigSolo::map_bigsolo_status(&series_data.release_status);
			let mut tags = series_data.tags.clone();
			if series_data.os.unwrap_or(false) {
				tags.push(String::from("One-shot"));
			}
			manga.tags = Some(tags);
			manga.url = Some(format!("https://bigsolo.org/{slug}"));

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
						let chapter_id = chapter
							.groups
							.values()
							.next()
							.and_then(|url| url.split('/').next_back())
							.unwrap_or("unknown");

						Chapter {
							key: String::from(chapter_id),
							title: Some(chapter.title),
							chapter_number: chapter_key.parse().ok(),
							volume_number: chapter.volume.parse().ok(),
							date_uploaded: Some(chapter.last_updated.parse().unwrap_or_default()),
							url: Some(format!("https://bigsolo.org/{slug}/{chapter_key}")),
							scanlators: {
								let mut scanlators: Vec<String> =
									chapter.groups.keys().cloned().collect();
								if let Some(collab) = chapter.collab.as_ref() {
									scanlators.push(collab.clone());
								}
								Some(scanlators)
							},
							locked: chapter.licencied.as_ref().is_some_and(|v| *v),
							..Default::default()
						}
					})
					.collect(),
			);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let response = Request::get(format!(
			"https://bigsolo.org/api/imgchest-chapter-pages?id={}",
			chapter.key
		))?
		.string()?;

		let parsed = serde_json::from_str::<ImageList>(&response).map_err(AidokuError::message)?;

		Ok(parsed
			.into_iter()
			.map(|image| Page {
				content: PageContent::url(image.link.clone()),
				has_description: false,
				thumbnail: Some(image.thumbnail.clone()),
				..Default::default()
			})
			.collect())
	}
}

impl BigSolo {
	// gets all manga list of the website
	fn get_bigsolo_manga_files() -> Vec<String> {
		let req = match Request::get("https://bigsolo.org/data/config.json")
			.ok()
			.and_then(|r| r.string().ok())
		{
			Some(r) => r,
			None => return Vec::new(),
		};

		let config: ConfigJson = match serde_json::from_str(&req).ok() {
			Some(cfg) => cfg,
			None => return Vec::new(),
		};

		// convert relative paths to absolute urls
		let urls: Vec<String> = config
			.LOCAL_SERIES_FILES
			.into_iter()
			.map(|file| format!("https://bigsolo.org/data/series/{file}"))
			.collect();

		urls
	}

	fn map_bigsolo_status(status: &str) -> MangaStatus {
		match status {
			"En cours" => MangaStatus::Ongoing,
			"Fini" | "Finis" => MangaStatus::Completed,
			"En pause" => MangaStatus::Hiatus,
			"Annulé" => MangaStatus::Cancelled,
			_ => MangaStatus::Unknown,
		}
	}
}

impl Home for BigSolo {
	fn get_home(&self) -> Result<HomeLayout> {
		let all_entries = self.get_search_manga_list(None, 1, Vec::new())?.entries;

		// load recommendations
		let reco_entries = match Request::get("https://bigsolo.org/data/reco.json")
			.ok()
			.and_then(|r| r.string().ok())
		{
			Some(response) => {
				match serde_json::from_str::<models::RecoFile>(&response) {
					Ok(recos) => {
						// match recommendations to manga data
						recos
							.into_iter()
							.filter_map(|reco| {
								let file_name = reco.file.replace(".json", "");
								all_entries
									.iter()
									.find(|manga| manga.key == file_name)
									.cloned()
							})
							.collect::<Vec<Manga>>()
					}
					Err(_) => Vec::new(),
				}
			}
			None => Vec::new(),
		};

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
					value: aidoku::HomeComponentValue::MangaList {
						ranking: false,
						page_size: None,
						entries: oneshot_entries.iter().cloned().map(|m| m.into()).collect(),
						listing: None,
					},
				},
				HomeComponent {
					title: Some(String::from("Liens")),
					subtitle: None,
					value: aidoku::HomeComponentValue::Links(vec![
						aidoku::Link {
							title: String::from("Colorisations"),
							value: Some(aidoku::LinkValue::Url(String::from(
								"https://bigsolo.org/galerie",
							))),
							..Default::default()
						},
						aidoku::Link {
							title: String::from("À propos de Big_herooooo"),
							value: Some(aidoku::LinkValue::Url(String::from(
								"https://bigsolo.org/presentation",
							))),
							..Default::default()
						},
					]),
				},
			],
		})
	}
}

impl AlternateCoverProvider for BigSolo {
	fn get_alternate_covers(&self, manga: Manga) -> Result<Vec<String>> {
		// reload the manga's json file to get covers_gallery
		let file_path = format!("https://bigsolo.org/data/series/{}.json", manga.key);

		let response = match Request::get(&file_path).ok().and_then(|r| r.string().ok()) {
			Some(r) => r,
			None => return Ok(Vec::new()),
		};

		let series_data: SeriesData = match serde_json::from_str(&response) {
			Ok(data) => data,
			Err(_) => return Ok(Vec::new()),
		};

		// extract covers from covers_gallery
		let covers: Vec<String> = series_data
			.covers_gallery
			.unwrap_or_default()
			.into_iter()
			.map(|cover| cover.url_hq)
			.collect();

		Ok(covers)
	}
}

// impl DeepLinkHandler for BigSolo {
//     fn handle_deep_link(&self, _url: String) -> Result<Option<DeepLinkResult>> {
//         // i'm not even gonna try to handle deep links, it's not worth the effort rn
//         // the website is getting reworked anyways, i'm not gonna waste my time on this
//         Ok(None)
//     }
// }

register_source!(BigSolo, Home, AlternateCoverProvider);
