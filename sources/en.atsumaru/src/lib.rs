#![no_std]
mod api;
mod filters;
mod helpers;
mod home;
mod models;
mod settings;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider, Manga,
	MangaPageResult, Page, PageContent, Result, Source,
	alloc::{String, Vec, vec},
	helpers::string::PlainText,
	imports::{
		net::{Request, TimeUnit, set_rate_limit},
		std::{current_date, send_partial_result},
	},
	prelude::*,
};
use api::{API_URL, CARD_FIELDS, RATING_FIELDS, SearchParams, get, lookup_url, parse, request};
use helpers::{arrange_chapters, is_novel, is_novel_url, page_url, parse_link};
use models::{
	ChaptersResponse, MangaPage, NovelResponse, PageResponse, ReadResponse, SearchResponse,
};

struct Atsumaru {
	seed: u32,
}

impl Source for Atsumaru {
	fn new() -> Self {
		set_rate_limit(10, 1, TimeUnit::Seconds);
		Self {
			seed: current_date().rem_euclid(2_147_483_646) as u32 + 1,
		}
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		if let Some((key, _)) = query.as_deref().map(str::trim).and_then(parse_link) {
			let found: SearchResponse = get(&lookup_url(&[&key], CARD_FIELDS))?;
			if !found.hits.is_empty() {
				return Ok(found.into_page(settings::label_novels()));
			}
		}

		let mut search = SearchParams::new(query);
		search.seed = self.seed;
		let mut tapped = Vec::new();
		for filter in filters {
			match filter {
				FilterValue::Sort {
					index, ascending, ..
				} => {
					search.sort = index as usize;
					search.ascending = ascending;
				}
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} => match id.as_str() {
					"genre" => {
						search.genres = included;
						search.excluded_genres.extend(excluded);
					}
					"demographic" => {
						search.demographics = included;
						search.excluded_tags.extend(excluded);
					}
					"type" => search.types = included,
					"rating" => {
						search.ratings = included;
						search.adult_only = false;
					}
					"status" => search.statuses = included,
					_ => {}
				},
				FilterValue::Select { id, value } => match id.as_str() {
					"genre_match" => search.any_genre = value == "any",
					"genre" => tapped.push(value),
					_ => {}
				},
				FilterValue::Text { id, value } if id == "author" => search.author = Some(value),
				FilterValue::Range { id, from, to } => match id.as_str() {
					"year" => search.years = (from, to),
					"chapters" => search.chapters = (from, to),
					_ => {}
				},
				FilterValue::Check { id, value } if id == "official" => {
					search.official = value == 1
				}
				_ => {}
			}
		}

		if !tapped.is_empty() {
			let (genres, tags) = filters::tag_ids(&tapped)?;
			search.genres.extend(genres);
			search.tags.extend(tags);
		}

		// A hidden genre or tag the search asks for is shown for that search.
		let genres = &search.genres;
		search.excluded_genres.retain(|id| !genres.contains(id));
		let (tags, demographics) = (&search.tags, &search.demographics);
		search
			.excluded_tags
			.retain(|id| !tags.contains(id) && !demographics.contains(id));

		search.page(page, settings::label_novels())
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let page_request = request(&format!("{API_URL}/manga/page?id={}", manga.key))?;
		// The page has no content rating, so details also look the series up in search.
		let (page, rating) = if needs_details {
			let rating_request = request(&lookup_url(&[&manga.key], RATING_FIELDS))?;
			let mut responses = Request::send_all([page_request, rating_request]).into_iter();
			let (Some(page), Some(rating)) = (responses.next(), responses.next()) else {
				bail!("Atsumaru didn't answer");
			};
			let rating = rating
				.ok()
				.and_then(|response| parse::<SearchResponse>(response).ok())
				.and_then(|search| search.hits.into_iter().next())
				.map(|hit| hit.document);
			(page?, rating)
		} else {
			(page_request.send()?, None)
		};
		let Some(MangaPage {
			info,
			chapters,
			scanlators,
			has_more_chapters,
		}) = parse::<PageResponse>(page)?.manga_page
		else {
			bail!("This series is no longer on Atsumaru");
		};
		let novel = is_novel(info.medium.as_deref());

		if needs_details {
			manga.copy_from(info.into_manga(settings::label_novels(), rating));
			if needs_chapters && has_more_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			// The series page carries only the first chapters of a long list.
			let chapters = if has_more_chapters {
				let url = format!("{API_URL}/manga/allChapters?mangaId={}", manga.key);
				get::<ChaptersResponse>(&url)?.chapters
			} else {
				chapters
			};
			let excluded = settings::excluded_scanlators();
			let chapters =
				arrange_chapters(chapters, &scanlators, &excluded, settings::deduplicate())
					.into_iter()
					.map(|chapter| chapter.into_chapter(&manga.key, novel, &scanlators))
					.collect();
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let novel = is_novel_url(chapter.url.as_deref()) || is_novel_url(manga.url.as_deref());
		let kind = if novel { "novelChapter" } else { "chapter" };
		let url = format!(
			"{API_URL}/read/{kind}?mangaId={}&chapterId={}",
			manga.key, chapter.key
		);

		if novel {
			let paragraphs = get::<NovelResponse>(&url)?.read_novel_chapter.paragraphs;
			let text = paragraphs
				.iter()
				.map(|paragraph| paragraph.trim())
				.filter(|paragraph| !paragraph.is_empty())
				.map(|paragraph| paragraph.escape_markdown())
				.collect::<Vec<_>>()
				.join("\n\n");
			if text.is_empty() {
				bail!("This chapter has no text");
			}
			return Ok(vec![Page {
				content: PageContent::text(text),
				..Default::default()
			}]);
		}

		let pages = get::<ReadResponse>(&url)?.read_chapter.pages;
		if pages.is_empty() {
			bail!("This chapter has no pages");
		}
		Ok(pages
			.into_iter()
			.map(|page| Page {
				content: PageContent::url(page_url(&page.image)),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for Atsumaru {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		home::listing_page(&listing.id, page)
	}
}

impl DeepLinkHandler for Atsumaru {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		Ok(parse_link(&url).map(|(manga_key, chapter)| match chapter {
			Some(key) => DeepLinkResult::Chapter { manga_key, key },
			None => DeepLinkResult::Manga { key: manga_key },
		}))
	}
}

register_source!(
	Atsumaru,
	Home,
	ListingProvider,
	DynamicSettings,
	DeepLinkHandler
);
