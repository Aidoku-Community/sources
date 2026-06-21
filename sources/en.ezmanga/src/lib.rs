#![no_std]

use aidoku::{
    Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, DynamicFilters,
    DynamicListings, Filter, FilterValue, Home, HomeComponent, HomeComponentValue, HomeLayout,
    Listing, ListingKind, ListingProvider, Link, Manga, MangaPageResult,
    Page, PageContent, Result, SelectFilter, Source, Viewer,
    alloc::{
        borrow::Cow,
        format,
        string::{String, ToString},
        vec,
        vec::Vec,
    },
    helpers::uri::{QueryParameters, encode_uri_component},
    imports::net::Request,
    prelude::*,
};

mod helpers;
mod models;

use helpers::*;
use models::*;

const API_BASE: &str = "https://vapi.ezmanga.org/api/v1";
const BASE_URL: &str = "https://ezmanga.org";

fn api_get(url: &str) -> Result<Request> {
    Ok(Request::get(url)?
        .header("Origin", BASE_URL)
        .header("Referer", "https://ezmanga.org/"))
}

struct EzManga;

impl Source for EzManga {
    fn new() -> Self {
        Self
    }

    fn get_search_manga_list(
        &self,
        query: Option<String>,
        page: i32,
        filters: Vec<FilterValue>,
    ) -> Result<MangaPageResult> {
        let url = match query.as_deref().filter(|q| !q.is_empty()) {
            Some(q) => format!(
                "{}/series/search?q={}&page={}",
                API_BASE,
                encode_uri_component(q),
                page
            ),
            None => {
                let mut qs = QueryParameters::new();
                qs.push("page", Some(&page.to_string()));
                for filter in &filters {
                    if let FilterValue::Select { id, value } = filter {
                        if !value.is_empty() {
                            qs.push(id, Some(value));
                        }
                    }
                }
                format!("{}/series?{}", API_BASE, qs)
            }
        };

        let resp: ApiList<ApiSeriesItem> = api_get(&url)?.json_owned()?;
        let has_next = resp.next.is_some();
        let entries = resp
            .data
            .into_iter()
            .filter(|s| s.series_type.as_deref() != Some("NOVEL"))
            .map(item_to_manga)
            .collect();

        Ok(MangaPageResult { entries, has_next_page: has_next })
    }

    fn get_manga_update(
        &self,
        mut manga: Manga,
        needs_details: bool,
        needs_chapters: bool,
    ) -> Result<Manga> {
        if needs_details {
            let det: ApiSeriesDetail =
                api_get(&format!("{}/series/{}", API_BASE, manga.key))?.json_owned()?;

            manga.title = String::from(det.title.trim());
            manga.cover = if det.cover.is_empty() { None } else { Some(det.cover) };
            manga.url = Some(format!("{}/series/{}", BASE_URL, det.slug));
            manga.status = parse_status(det.status.as_deref());
            manga.content_rating = ContentRating::Safe;
            manga.viewer = Viewer::Webtoon;

            if let Some(raw_desc) = det.description {
                let desc = strip_html(&raw_desc);
                if !desc.is_empty() {
                    manga.description = Some(desc);
                }
            }

            let mut authors: Vec<String> = Vec::new();
            if let Some(a) = det.author.filter(|s| !s.is_empty()) {
                authors.push(a);
            }
            if let Some(a) = det.artist.filter(|s| !s.is_empty()) {
                if !authors.contains(&a) {
                    authors.push(a);
                }
            }
            if !authors.is_empty() {
                manga.authors = Some(authors);
            }

            if let Some(genres) = det.genres {
                let tags: Vec<String> = genres.into_iter().map(|g| g.name).collect();
                if !tags.is_empty() {
                    manga.tags = Some(tags);
                }
            }
        }

        if needs_chapters {
            let mut chapters = Vec::new();
            let mut page = 1i32;

            loop {
                let resp: ApiList<ApiChapter> = api_get(&format!(
                    "{}/series/{}/chapters?page={}",
                    API_BASE, manga.key, page
                ))?
                .json_owned()?;

                let has_next = resp.next.is_some();

                for ch in resp.data {
                    if ch.locked {
                        continue;
                    }
                    chapters.push(Chapter {
                        key: ch.slug,
                        chapter_number: Some(ch.number as f32),
                        title: ch.title.filter(|t| !t.is_empty()),
                        date_uploaded: ch.created_at.as_deref().and_then(parse_date),
                        ..Default::default()
                    });
                }

                if !has_next {
                    break;
                }
                page += 1;
            }

            manga.chapters = Some(chapters);
        }

        Ok(manga)
    }

    fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
        let det: ApiChapterDetail = api_get(&format!(
            "{}/series/{}/chapters/{}",
            API_BASE, manga.key, chapter.key
        ))?
        .json_owned()?;

        let pages = det
            .images
            .into_iter()
            .map(|img| Page {
                content: PageContent::url(img.url),
                ..Default::default()
            })
            .collect();

        Ok(pages)
    }
}

impl ListingProvider for EzManga {
    fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
        let sort = if listing.id == "latest" { "latest" } else { "popular" };
        let resp: ApiList<ApiSeriesItem> = api_get(&format!(
            "{}/series?page={}&sort={}",
            API_BASE, page, sort
        ))?
        .json_owned()?;

        let has_next = resp.next.is_some();
        let entries = resp
            .data
            .into_iter()
            .filter(|s| s.series_type.as_deref() != Some("NOVEL"))
            .map(item_to_manga)
            .collect();

        Ok(MangaPageResult { entries, has_next_page: has_next })
    }
}

impl DynamicListings for EzManga {
    fn get_dynamic_listings(&self) -> Result<Vec<Listing>> {
        Ok(vec![
            Listing {
                id: String::from("popular"),
                name: String::from("Popular"),
                kind: ListingKind::Default,
            },
            Listing {
                id: String::from("latest"),
                name: String::from("Latest"),
                kind: ListingKind::Default,
            },
        ])
    }
}

impl DynamicFilters for EzManga {
    fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
        Ok(vec![
            SelectFilter {
                id: Cow::Borrowed("sort"),
                title: Some(Cow::Borrowed("Sort By")),
                options: vec![
                    Cow::Borrowed("Default"),
                    Cow::Borrowed("Popular"),
                    Cow::Borrowed("Latest"),
                ],
                ids: Some(vec![
                    Cow::Borrowed(""),
                    Cow::Borrowed("popular"),
                    Cow::Borrowed("latest"),
                ]),
                ..Default::default()
            }
            .into(),
            SelectFilter {
                id: Cow::Borrowed("status"),
                title: Some(Cow::Borrowed("Status")),
                options: vec![
                    Cow::Borrowed("Any"),
                    Cow::Borrowed("Ongoing"),
                    Cow::Borrowed("Completed"),
                    Cow::Borrowed("Dropped"),
                    Cow::Borrowed("Hiatus"),
                ],
                ids: Some(vec![
                    Cow::Borrowed(""),
                    Cow::Borrowed("ONGOING"),
                    Cow::Borrowed("COMPLETED"),
                    Cow::Borrowed("DROPPED"),
                    Cow::Borrowed("HIATUS"),
                ]),
                ..Default::default()
            }
            .into(),
            SelectFilter {
                id: Cow::Borrowed("type"),
                title: Some(Cow::Borrowed("Type")),
                options: vec![
                    Cow::Borrowed("Any"),
                    Cow::Borrowed("Manhwa"),
                    Cow::Borrowed("Manga"),
                    Cow::Borrowed("Manhua"),
                    Cow::Borrowed("Novel"),
                ],
                ids: Some(vec![
                    Cow::Borrowed(""),
                    Cow::Borrowed("MANHWA"),
                    Cow::Borrowed("MANGA"),
                    Cow::Borrowed("MANHUA"),
                    Cow::Borrowed("NOVEL"),
                ]),
                ..Default::default()
            }
            .into(),
        ])
    }
}

impl Home for EzManga {
    fn get_home(&self) -> Result<HomeLayout> {
        let popular_listing = Listing {
            id: String::from("popular"),
            name: String::from("Popular"),
            kind: ListingKind::Default,
        };
        let latest_listing = Listing {
            id: String::from("latest"),
            name: String::from("Latest"),
            kind: ListingKind::Default,
        };

        let popular_resp: ApiList<ApiSeriesItem> =
            api_get(&format!("{}/series?page=1&sort=popular", API_BASE))?.json_owned()?;
        let popular_links: Vec<Link> = popular_resp
            .data
            .into_iter()
            .filter(|s| s.series_type.as_deref() != Some("NOVEL"))
            .map(item_to_link)
            .collect();

        let latest_resp: ApiList<ApiSeriesItem> =
            api_get(&format!("{}/series?page=1&sort=latest", API_BASE))?.json_owned()?;
        let latest_links: Vec<Link> = latest_resp
            .data
            .into_iter()
            .filter(|s| s.series_type.as_deref() != Some("NOVEL"))
            .map(item_to_link)
            .collect();

        Ok(HomeLayout {
            components: vec![
                HomeComponent {
                    title: Some(String::from("Popular")),
                    subtitle: None,
                    value: HomeComponentValue::Scroller {
                        entries: popular_links,
                        listing: Some(popular_listing),
                    },
                },
                HomeComponent {
                    title: Some(String::from("Latest")),
                    subtitle: None,
                    value: HomeComponentValue::Scroller {
                        entries: latest_links,
                        listing: Some(latest_listing),
                    },
                },
            ],
        })
    }
}

impl DeepLinkHandler for EzManga {
    fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
        let prefix = "https://ezmanga.org/series/";
        if let Some(rest) = url.strip_prefix(prefix) {
            let slug = rest.split('/').next().unwrap_or(rest);
            if !slug.is_empty() {
                return Ok(Some(DeepLinkResult::Manga {
                    key: String::from(slug),
                }));
            }
        }
        Ok(None)
    }
}

register_source!(EzManga, ListingProvider, DynamicListings, DynamicFilters, Home, DeepLinkHandler);
