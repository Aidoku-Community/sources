#![no_std]

use aidoku::{
    Chapter, ContentRating, DeepLinkHandler, DeepLinkResult,
    FilterValue, Home, HomeComponent, HomeComponentValue, HomeLayout,
    ImageRequestProvider, Link, Listing, ListingKind, ListingProvider, Manga, MangaPageResult,
    MangaStatus, Page, PageContent, PageContext, Result, Source, Viewer,
    alloc::{
        collections::BTreeMap,
        format,
        string::String,
        vec,
        vec::Vec,
    },
    imports::net::{Request, TimeUnit, set_rate_limit},
    prelude::*,
};

mod helpers;
mod models;
use helpers::strip_html;
use models::*;

pub const BASE_URL: &str = "https://danke.moe";
const API_BASE: &str = "https://danke.moe/api";
const PAGE_SIZE: usize = 20;

const USER_AGENT: &str = "aidoku/1.0 CFNetwork/1490.0.4 Darwin/23.4.0";

fn api_get(url: &str) -> Result<Request> {
    Ok(Request::get(url)?
        .header("Accept", "application/json")
        .header("User-Agent", USER_AGENT))
}

fn fetch_all_series() -> Result<Vec<(String, AllSeriesItem)>> {
    let map: BTreeMap<String, AllSeriesItem> =
        api_get(&format!("{API_BASE}/get_all_series/"))?.json_owned()?;
    Ok(map.into_iter().filter(|(_, v)| !v.slug.is_empty()).collect())
}

struct DankeMoe;

impl Source for DankeMoe {
    fn new() -> Self {
        set_rate_limit(2, 2, TimeUnit::Seconds);
        Self
    }

    fn get_search_manga_list(
        &self,
        query: Option<String>,
        page: i32,
        _filters: Vec<FilterValue>,
    ) -> Result<MangaPageResult> {
        let mut all = fetch_all_series()?;

        if let Some(q) = &query {
            let q_lower = q.to_lowercase();
            all.retain(|(title, _)| title.to_lowercase().contains(&q_lower));
        }

        all.sort_by(|a, b| b.1.last_updated.cmp(&a.1.last_updated));

        let start = ((page - 1) as usize) * PAGE_SIZE;
        let has_next_page = start + PAGE_SIZE < all.len();
        let entries = all
            .into_iter()
            .skip(start)
            .take(PAGE_SIZE)
            .map(|(title, item)| item.into_manga(title))
            .collect();

        Ok(MangaPageResult { entries, has_next_page })
    }

    fn get_manga_update(
        &self,
        mut manga: Manga,
        needs_details: bool,
        needs_chapters: bool,
    ) -> Result<Manga> {
        if needs_details || needs_chapters {
            let det: SeriesDetail =
                api_get(&format!("{API_BASE}/series/{}/", manga.key))?.json_owned()?;

            if needs_details {
                manga.title = String::from(det.title.trim());
                manga.cover = if det.cover.is_empty() {
                    None
                } else {
                    Some(format!("{BASE_URL}{}", det.cover))
                };
                manga.url = Some(format!("{BASE_URL}/reader/series/{}/", det.slug));
                manga.status = MangaStatus::Unknown;
                manga.content_rating = ContentRating::Suggestive;
                manga.viewer = Viewer::RightToLeft;

                let desc = strip_html(&det.description);
                if !desc.is_empty() {
                    manga.description = Some(desc);
                }
                if !det.author.is_empty() {
                    manga.authors = Some(vec![det.author.clone()]);
                }
                if !det.artist.is_empty() && det.artist != det.author {
                    manga.artists = Some(vec![det.artist.clone()]);
                }
            }

            if needs_chapters {
                let chapters = det
                    .chapters
                    .0
                    .iter()
                    .filter(|(_, ch)| ch.is_public)
                    .map(|(num_str, ch)| {
                        let scanlators: Vec<String> = ch
                            .groups
                            .group_ids()
                            .filter_map(|gid| det.groups.get(gid))
                            .map(String::from)
                            .collect();
                        Chapter {
                            key: num_str.clone(),
                            chapter_number: num_str.parse().ok(),
                            title: ch.title.clone().filter(|t| !t.is_empty()),
                            date_uploaded: ch.release_date.0,
                            scanlators: if scanlators.is_empty() { None } else { Some(scanlators) },
                            url: Some(format!(
                                "{BASE_URL}/reader/series/{}/{}/1/",
                                det.slug, num_str
                            )),
                            ..Default::default()
                        }
                    })
                    .collect();
                manga.chapters = Some(chapters);
            }
        }

        Ok(manga)
    }

    fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
        let det: SeriesDetail =
            api_get(&format!("{API_BASE}/series/{}/", manga.key))?.json_owned()?;

        let ch = match det.chapters.find(&chapter.key) {
            Some(ch) => ch,
            None => return Ok(Vec::new()),
        };

        let pages = match ch.groups.0.first() {
            Some((group_id, filenames)) => filenames
                .iter()
                .map(|filename| Page {
                    content: PageContent::url(format!(
                        "{BASE_URL}/media/manga/{}/chapters/{}/{}/{}",
                        manga.key, ch.folder, group_id, filename
                    )),
                    ..Default::default()
                })
                .collect(),
            None => Vec::new(),
        };

        Ok(pages)
    }
}

impl ListingProvider for DankeMoe {
    fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
        let mut all = fetch_all_series()?;

        match listing.id.as_str() {
            "Latest" => all.sort_by(|a, b| b.1.last_updated.cmp(&a.1.last_updated)),
            _ => all.sort_by(|a, b| a.0.cmp(&b.0)),
        }

        let start = ((page - 1) as usize) * PAGE_SIZE;
        let has_next_page = start + PAGE_SIZE < all.len();
        let entries = all
            .into_iter()
            .skip(start)
            .take(PAGE_SIZE)
            .map(|(title, item)| item.into_manga(title))
            .collect();

        Ok(MangaPageResult { entries, has_next_page })
    }
}

impl Home for DankeMoe {
    fn get_home(&self) -> Result<HomeLayout> {
        let mut all = fetch_all_series()?;
        all.sort_by(|a, b| b.1.last_updated.cmp(&a.1.last_updated));

        let entries: Vec<Link> = all
            .into_iter()
            .take(20)
            .map(|(title, item)| item.into_manga(title).into())
            .collect();

        Ok(HomeLayout {
            components: vec![HomeComponent {
                title: Some(String::from("Latest Updates")),
                subtitle: None,
                value: HomeComponentValue::Scroller {
                    entries,
                    listing: Some(Listing {
                        id: String::from("Latest"),
                        name: String::from("Latest"),
                        kind: ListingKind::Default,
                    }),
                },
            }],
        })
    }
}

impl DeepLinkHandler for DankeMoe {
    fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
        let Some(path) = url.strip_prefix(BASE_URL) else {
            return Ok(None);
        };
        let rest = path
            .strip_prefix("/reader/series/")
            .or_else(|| path.strip_prefix("/read/series/"));
        let Some(rest) = rest else {
            return Ok(None);
        };
        let slug = rest.split('/').next().unwrap_or(rest);
        let slug = slug.split('?').next().unwrap_or(slug);
        let slug = slug.split('#').next().unwrap_or(slug);
        if slug.is_empty() {
            return Ok(None);
        }
        Ok(Some(DeepLinkResult::Manga { key: String::from(slug) }))
    }
}

impl ImageRequestProvider for DankeMoe {
    fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
        api_get(&url)
    }
}

register_source!(DankeMoe, ListingProvider, Home, DeepLinkHandler, ImageRequestProvider);
