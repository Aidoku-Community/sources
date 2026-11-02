#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent, HomeComponentValue,
	HomeLayout, Link, Manga, MangaPageResult, Page, PageContent, Result, Source,
	alloc::{String, Vec, vec},
	helpers::uri::encode_uri_component,
	imports::std::current_date,
	prelude::*,
};

mod helpers;
mod models;
mod settings;

use helpers::{SearchParams, locked_chapters, markdown, next_data};
use models::{Block, BrowseProps, ChapterProps, Entry, HomeProps, SeriesProps, Slide};

const BASE_URL: &str = "https://flamecomics.xyz";
const CDN_URL: &str = "https://cdn.flamecomics.xyz";

struct FlameComics;

impl Source for FlameComics {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		_page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut params = SearchParams {
			query: query.map(|query| query.to_lowercase()),
			..Default::default()
		};
		for filter in filters {
			match filter {
				FilterValue::Sort {
					index, ascending, ..
				} => {
					params.popular = index == 1;
					params.descending = !ascending;
				}
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} => match id.as_str() {
					"genre" => {
						params.genres = included;
						params.hidden_genres = excluded;
					}
					"type" => params.types = included,
					"status" => params.statuses = included,
					"year" => params.years = included,
					_ => {}
				},
				_ => {}
			}
		}
		let mut series = next_data::<BrowseProps>("/browse")?.series;
		series.retain(|entry| params.matches(entry));
		// As on the site: descending popularity starts at rank 1, and ties stay A to Z.
		series.sort_by_cached_key(|entry| {
			let rank = params.popular.then(|| {
				let rank = i64::from(entry.popularity_rank.unwrap_or(u32::MAX));
				if params.descending { rank } else { -rank }
			});
			(rank, entry.title.to_lowercase())
		});
		if params.descending && !params.popular {
			series.reverse();
		}
		Ok(MangaPageResult {
			entries: series
				.into_iter()
				.filter_map(|entry| Manga::try_from(entry).ok())
				.collect(),
			has_next_page: false,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let SeriesProps { series, chapters } = next_data(&manga.key)?;
		if needs_chapters {
			let locks = locked_chapters(&series.unlock, &chapters, current_date());
			let show_locked = settings::show_locked();
			let url = format!("{BASE_URL}{}", manga.key);
			manga.chapters = Some(
				chapters
					.into_iter()
					.zip(locks)
					.filter(|(_, locked)| show_locked || !locked)
					.map(|(chapter, locked)| chapter.into_chapter(&url, locked))
					.collect(),
			);
		}
		if needs_details {
			manga.copy_from(series.into_details()?);
		}
		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let reader = next_data::<ChapterProps>(&format!("{}{}", manga.key, chapter.key))?.chapter;
		if manga.key.starts_with("/novel/") {
			let Some(content) = reader.content else {
				bail!("This chapter is in early access on Flame Comics");
			};
			let text = markdown(&content);
			if text.is_empty() {
				bail!("This chapter has no text");
			}
			return Ok(vec![Page {
				content: PageContent::text(text),
				..Default::default()
			}]);
		}
		// Image folders follow the series and chapter paths. The site's reader adds the edit
		// time, so re-uploaded pages aren't served from a cache.
		let pages: Vec<Page> = reader
			.images
			.into_values()
			.map(|image| Page {
				content: PageContent::url(format!(
					"{CDN_URL}/uploads/images{}{}/{}?{}",
					manga.key,
					chapter.key,
					encode_uri_component(image.name),
					reader.edit_time.unwrap_or_default()
				)),
				..Default::default()
			})
			.collect();
		if pages.is_empty() {
			bail!("This chapter has no pages");
		}
		Ok(pages)
	}
}

impl Home for FlameComics {
	fn get_home(&self) -> Result<HomeLayout> {
		let home = next_data::<HomeProps>("/")?;
		let mut components = Vec::new();

		let banners: Vec<Link> = home
			.carousel
			.into_iter()
			.filter_map(Slide::into_link)
			.collect();
		if !banners.is_empty() {
			components.push(HomeComponent {
				title: Some("Featured".into()),
				subtitle: None,
				// The banners are about 8:3; a fixed size smooths preloading.
				value: HomeComponentValue::ImageScroller {
					links: banners,
					auto_scroll_interval: Some(6.0),
					width: Some(400),
					height: Some(150),
				},
			});
		}

		let scroller = |block: Block| HomeComponent {
			title: Some(block.title),
			subtitle: None,
			value: HomeComponentValue::Scroller {
				entries: block
					.series
					.into_iter()
					.filter_map(|entry| Manga::try_from(entry).ok())
					.map(Into::into)
					.collect(),
				listing: None,
			},
		};
		let latest = |block: Block| HomeComponent {
			title: Some(block.title),
			subtitle: None,
			value: HomeComponentValue::MangaChapterList {
				page_size: Some(5),
				entries: block
					.series
					.into_iter()
					.filter_map(Entry::into_latest)
					.collect(),
				listing: None,
			},
		};
		components.extend(home.popular.into_block().map(scroller));
		components.extend(home.latest.into_block().map(|block| {
			latest(Block {
				title: "Latest Updates".into(),
				..block
			})
		}));
		components.extend(home.novels.into_block().map(scroller));
		components.extend(home.staff_picks.into_block().map(scroller));
		components.extend(home.latest_novels.into_block().map(latest));
		Ok(HomeLayout { components })
	}
}

impl DeepLinkHandler for FlameComics {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};
		let path = path.split(['?', '#']).next().unwrap_or_default();
		let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
		Ok(match segments.as_slice() {
			[kind @ ("series" | "novel"), id] => Some(DeepLinkResult::Manga {
				key: format!("/{kind}/{id}"),
			}),
			[kind @ ("series" | "novel"), id, token] => Some(DeepLinkResult::Chapter {
				manga_key: format!("/{kind}/{id}"),
				key: format!("/{token}"),
			}),
			_ => None,
		})
	}
}

register_source!(FlameComics, Home, DeepLinkHandler);
