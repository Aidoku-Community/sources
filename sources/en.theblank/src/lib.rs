#![no_std]

mod helpers;
mod models;

use aidoku::{
	AidokuError, Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Home,
	HomeLayout, Listing, ListingProvider, Manga, MangaPageResult, MangaStatus, Page, PageContent,
	Result, Source, Viewer,
	alloc::{String, Vec, format, vec},
	helpers::uri::QueryParameters,
	imports::std::current_date,
	prelude::*,
};

use helpers::{abs_url, build_page_url, fetch_html, parse_chapter_date, parse_inertia};
use models::{ChapterReaderProps, LibraryProps, LibrarySerie, SerieChapter, SerieDetailProps};

pub const BASE_URL: &str = "https://theblank.net";

// ─── MangaStatus helper ───────────────────────────────────────────────────────

fn manga_status(s: &str) -> MangaStatus {
	match s {
		"ongoing" => MangaStatus::Ongoing,
		"finished" => MangaStatus::Completed,
		"dropped" => MangaStatus::Cancelled,
		"onhold" => MangaStatus::Hiatus,
		_ => MangaStatus::Unknown,
	}
}

// ─── From impls ───────────────────────────────────────────────────────────────

impl From<LibrarySerie> for Manga {
	fn from(s: LibrarySerie) -> Self {
		Manga {
			key: s.link,
			title: s.title,
			cover: Some(abs_url(&s.image)),
			..Default::default()
		}
	}
}

/// Wrapper to carry serie_slug into the Chapter From impl.
pub struct ChapterWithSlug<'a> {
	pub chapter: SerieChapter,
	pub serie_slug: &'a str,
}

impl<'a> From<ChapterWithSlug<'a>> for Chapter {
	fn from(w: ChapterWithSlug<'a>) -> Self {
		let c = w.chapter;
		let key = format!("{}|{}", w.serie_slug, c.slug);
		Chapter {
			key,
			title: Some(c.title),
			chapter_number: Some(c.chapter_number),
			date_uploaded: parse_chapter_date(&c.created_at),
			thumbnail: c.thumbnail.map(|t| abs_url(&t)),
			url: Some(format!(
				"{BASE_URL}/serie/{}/chapter/{}",
				w.serie_slug, c.slug
			)),
			..Default::default()
		}
	}
}

// ─── Source ───────────────────────────────────────────────────────────────────

struct TheBlank;

impl Source for TheBlank {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut params = QueryParameters::new();
		params.push_encoded("page", Some(&format!("{page}")));

		if let Some(q) = &query {
			params.push("search", Some(q));
		}

		for filter in &filters {
			match filter {
				FilterValue::Select { id, value } => match id.as_str() {
					"orderby" => {
						params.push_encoded("orderby", Some(value));
					}
					"status" if !value.is_empty() => {
						params.push_encoded("status[]", Some(value));
					}
					_ => {}
				},
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} if id == "genres" => {
					for g in included {
						params.push_encoded("include_genres[]", Some(g));
					}
					for g in excluded {
						params.push_encoded("exclude_genres[]", Some(g));
					}
				}
				_ => {}
			}
		}

		let url = format!("{BASE_URL}/library?{params}");
		let html = fetch_html(&url)?;
		let props: LibraryProps = parse_inertia(&html).ok_or(AidokuError::Message(
			String::from("failed to parse library"),
		))?;

		let has_next_page = props.series.meta.current_page < props.series.meta.last_page;
		let entries = props.series.data.into_iter().map(Into::into).collect();

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn get_manga_update(
		&self,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{BASE_URL}{}", manga.key);
		let html = fetch_html(&url)?;
		let props: SerieDetailProps = parse_inertia(&html).ok_or(AidokuError::Message(
			String::from("failed to parse serie page"),
		))?;

		let s = props.serie;

		let (cover, description, authors, tags, status, content_rating, viewer) = if needs_details {
			let tags: Vec<String> = s.genres.into_iter().map(|g| g.name).collect();
			(
				Some(abs_url(&s.cover_image)),
				Some(s.description),
				Some(vec![s.author]),
				Some(tags),
				manga_status(&s.status),
				ContentRating::NSFW,
				Viewer::Webtoon,
			)
		} else {
			(
				None,
				None,
				None,
				None,
				MangaStatus::Unknown,
				ContentRating::Safe,
				Viewer::default(),
			)
		};

		let chapters = if needs_chapters {
			Some(
				s.chapters
					.into_iter()
					.map(|c| {
						ChapterWithSlug {
							chapter: c,
							serie_slug: &s.slug,
						}
						.into()
					})
					.collect(),
			)
		} else {
			None
		};

		Ok(Manga {
			key: manga.key,
			title: s.name,
			cover,
			description,
			authors,
			tags,
			status,
			content_rating,
			viewer,
			chapters,
			..Default::default()
		})
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		// chapter.key = "{serie_slug}|{chapter_slug}"
		let sep = chapter
			.key
			.find('|')
			.ok_or(AidokuError::Message(String::from("bad chapter key")))?;
		let serie_slug = &chapter.key[..sep];
		let chapter_slug = &chapter.key[sep + 1..];

		let url = format!("{BASE_URL}/serie/{serie_slug}/chapter/{chapter_slug}");
		let html = fetch_html(&url)?;
		let props: ChapterReaderProps = parse_inertia(&html).ok_or(AidokuError::Message(
			String::from("failed to parse chapter page"),
		))?;

		let d = props.data;
		let sr_slug = d.serie.slug;
		let ch_slug = d.slug;
		let token = d.chapter_token;
		let ts = current_date();

		let pages = (1..=d.page_count)
			.map(|i| {
				let mut nonce = [0u8; 8];
				let seed = (ts as u64)
					.wrapping_add(i as u64)
					.wrapping_mul(0x9e3779b97f4a7c15);
				nonce.copy_from_slice(&seed.to_le_bytes());
				Page {
					content: PageContent::url(build_page_url(
						&sr_slug, &ch_slug, &token, i, ts, &nonce,
					)),
					..Default::default()
				}
			})
			.collect();

		Ok(pages)
	}
}

// ─── Listing provider ─────────────────────────────────────────────────────────

impl ListingProvider for TheBlank {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let orderby = match listing.id.as_str() {
			"trending" => "trending",
			"recently" => "recently",
			"views" => "views",
			"alphabetical" => "alphabetical",
			_ => "date",
		};
		let url = format!("{BASE_URL}/library?page={page}&orderby={orderby}");
		let html = fetch_html(&url)?;
		let props: LibraryProps = parse_inertia(&html).ok_or(AidokuError::Message(
			String::from("failed to parse library"),
		))?;

		let has_next_page = props.series.meta.current_page < props.series.meta.last_page;
		let entries = props.series.data.into_iter().map(Into::into).collect();

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}
}

// ─── Home ─────────────────────────────────────────────────────────────────────

impl Home for TheBlank {
	fn get_home(&self) -> Result<HomeLayout> {
		Err(AidokuError::Unimplemented)
	}
}

// ─── Deep link ────────────────────────────────────────────────────────────────

impl DeepLinkHandler for TheBlank {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url.strip_prefix(BASE_URL).unwrap_or(url.as_str());

		if let Some(rest) = path.strip_prefix("/serie/") {
			if let Some(ch_idx) = rest.find("/chapter/") {
				let serie_slug = &rest[..ch_idx];
				let chapter_slug = &rest[ch_idx + "/chapter/".len()..];
				return Ok(Some(DeepLinkResult::Chapter {
					manga_key: format!("/serie/{serie_slug}"),
					key: format!("{serie_slug}|{chapter_slug}"),
				}));
			}
			return Ok(Some(DeepLinkResult::Manga {
				key: format!("/serie/{rest}"),
			}));
		}

		Ok(None)
	}
}

register_source!(TheBlank, ListingProvider, Home, DeepLinkHandler);
