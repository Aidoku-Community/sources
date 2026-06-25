#![no_std]

use aidoku::{
    Chapter, ContentRating, DeepLinkHandler, DeepLinkResult,
    FilterValue, Home, HomeComponent, HomeComponentValue, HomeLayout,
    Link, Listing, ListingKind, ListingProvider, Manga, MangaPageResult, MangaWithChapter,
    Page, PageContent, Result, Source, Viewer,
    alloc::{
        format,
        string::{String, ToString},
        vec,
        vec::Vec,
    },
    helpers::uri::{QueryParameters, encode_uri_component},
    imports::{net::Request, std::parse_date},
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
        let url = match query.as_deref() {
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
                    if let FilterValue::Select { id, value } = filter
                        && !value.is_empty()
                    {
                        qs.push(id, Some(value));
                    }
                }
                format!("{}/series?{}", API_BASE, qs)
            }
        };

        let resp: ApiList<ApiSeriesItem> = api_get(&url)?.json_owned()?;
        let has_next_page = resp.next.is_some();
        let entries = resp
            .data
            .into_iter()
            .filter(|s| s.series_type.as_deref() != Some("NOVEL"))
            .map(Manga::from)
            .collect();

        Ok(MangaPageResult { entries, has_next_page })
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

            if let Some(a) = det.author.filter(|s| !s.is_empty()) {
                manga.authors = Some(vec![a]);
            }
            if let Some(a) = det.artist.filter(|s| !s.is_empty()) {
                manga.artists = Some(vec![a]);
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
                    if !ch.is_free {
                        continue;
                    }
                    chapters.push(Chapter {
                        key: ch.slug,
                        chapter_number: Some(ch.number as f32),
                        title: ch.title.filter(|t| !t.is_empty()),
                        date_uploaded: ch.created_at.as_deref().and_then(|s| {
                            let s = s.split_once('.').map_or(s, |(before, _)| before);
                            parse_date(format!("{s}Z"), "yyyy-MM-dd'T'HH:mm:ss'Z'")
                        }),
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
        let sort = if listing.id == "Latest" { "latest" } else { "popular" };
        let resp: ApiList<ApiSeriesItem> = api_get(&format!(
            "{}/series?page={}&sort={}",
            API_BASE, page, sort
        ))?
        .json_owned()?;

        let has_next_page = resp.next.is_some();
        let entries = resp
            .data
            .into_iter()
            .filter(|s| s.series_type.as_deref() != Some("NOVEL"))
            .map(Manga::from)
            .collect();

        Ok(MangaPageResult { entries, has_next_page })
    }
}

impl Home for EzManga {
    fn get_home(&self) -> Result<HomeLayout> {
        let resp: ApiHomeResponse =
            api_get(&format!("{}/home", API_BASE))?.json_owned()?;

        let filter_novels =
            |s: &ApiSeriesItem| s.series_type.as_deref() != Some("NOVEL");

        let to_entry = |s: ApiSeriesItem| -> Option<MangaWithChapter> {
            let ch = s.chapters.first()?;
            let chapter = Chapter {
                key: ch.slug.clone(),
                chapter_number: Some(ch.number as f32),
                ..Default::default()
            };
            Some(MangaWithChapter { manga: Manga::from(s), chapter })
        };

        let popular: Vec<Link> = resp.popular
            .into_iter()
            .filter(filter_novels)
            .map(|s| Manga::from(s).into())
            .collect();

        let pinned = resp.pinned
            .into_iter()
            .filter(filter_novels)
            .filter_map(to_entry)
            .collect::<Vec<_>>();

        let latest = resp.new_series
            .into_iter()
            .filter(filter_novels)
            .filter_map(to_entry)
            .collect::<Vec<_>>();

        Ok(HomeLayout {
            components: vec![
                HomeComponent {
                    title: Some(String::from("Popular Today")),
                    subtitle: None,
                    value: HomeComponentValue::Scroller {
                        entries: popular,
                        listing: Some(Listing {
                            id: String::from("Popular"),
                            name: String::from("Popular"),
                            kind: ListingKind::Default,
                        }),
                    },
                },
                HomeComponent {
                    title: Some(String::from("Pinned Series")),
                    subtitle: None,
                    value: HomeComponentValue::MangaChapterList {
                        entries: pinned,
                        page_size: None,
                        listing: None,
                    },
                },
                HomeComponent {
                    title: Some(String::from("Latest Updates")),
                    subtitle: None,
                    value: HomeComponentValue::MangaChapterList {
                        entries: latest,
                        page_size: None,
                        listing: Some(Listing {
                            id: String::from("Latest"),
                            name: String::from("Latest"),
                            kind: ListingKind::Default,
                        }),
                    },
                },
            ],
        })
    }
}

impl DeepLinkHandler for EzManga {
    fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
        let prefix = format!("{}/series/", BASE_URL);
        if let Some(rest) = url.strip_prefix(&prefix) {
            let slug = rest.split('/').next().unwrap_or(rest);
            let slug = slug.split('?').next().unwrap_or(slug);
            let slug = slug.split('#').next().unwrap_or(slug);
            if !slug.is_empty() {
                return Ok(Some(DeepLinkResult::Manga {
                    key: String::from(slug),
                }));
            }
        }
        Ok(None)
    }
}

register_source!(EzManga, ListingProvider, Home, DeepLinkHandler);
