#![no_std]
use aidoku::{
    Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent, HomeComponentValue,
    HomeLayout, ImageRequestProvider, Link, Listing, ListingKind, ListingProvider, Manga,
    MangaPageResult, MangaStatus, MangaWithChapter, Page, PageContent, PageContext, Result, Source,
    Viewer,
    alloc::{String, Vec, string::ToString, vec},
    helpers::uri::QueryParameters,
    imports::{html::Document, net::Request},
    prelude::*,
};
use chrono::{DateTime, Utc};
use serde_json::Value;

const BASE_URL: &str = "https://toontop.io";
const API_URL: &str = "https://api.toontop.io";
const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1";

struct ToonTop;

fn get(url: &str) -> Result<Document> {
    Ok(Request::get(url)?.header("User-Agent", USER_AGENT).html()?)
}

fn next_data(html: &Document) -> Result<Value> {
    let script = html
        .select_first("script#__NEXT_DATA__")
        .ok_or_else(|| error!("Missing __NEXT_DATA__"))?;
    let text = script
        .data()
        .or_else(|| script.html())
        .ok_or_else(|| error!("Empty __NEXT_DATA__"))?;
    Ok(serde_json::from_str::<Value>(&text)?)
}

fn manga_from_value(item: &Value) -> Option<Manga> {
    let slug = item["url"].as_str()?;
    let title = item["name"].as_str()?.to_string();
    let cover = item["cover"].as_str().map(String::from);
    Some(Manga {
        key: slug.to_string(),
        title,
        cover,
        url: Some(format!("{BASE_URL}{slug}")),
        ..Default::default()
    })
}

fn manga_with_chapter_from_value(item: &Value) -> Option<MangaWithChapter> {
    let manga = manga_from_value(item)?;
    let ch = &item["latestChapters"][0];
    let ch_name = ch["name"].as_str().unwrap_or_default();
    let ch_slug = ch["slug"].as_str().filter(|s| !s.is_empty())?;
    let chapter_number = ch_name
        .split_whitespace()
        .nth(1)
        .and_then(|n| n.parse::<f32>().ok());
    Some(MangaWithChapter {
        manga,
        chapter: Chapter {
            key: format!("/{ch_slug}"),
            chapter_number,
            ..Default::default()
        },
    })
}

fn parse_chapter(c: &Value) -> Option<Chapter> {
    let slug = c["slug"].as_str()?;
    let chapter_url = c["url"].as_str().unwrap_or_default();
    let name = c["name"].as_str().unwrap_or_default();
    let chapter_number = name
        .split_whitespace()
        .nth(1)
        .and_then(|n| n.parse::<f32>().ok());
    let date = c["updated_at"]
        .as_str()
        .and_then(|s| s.parse::<DateTime<Utc>>().ok())
        .map(|d| d.timestamp());
    Some(Chapter {
        key: format!("/{slug}"),
        chapter_number,
        date_uploaded: date,
        url: Some(format!("{BASE_URL}{chapter_url}")),
        ..Default::default()
    })
}

fn items_from_array(arr: &Value) -> Vec<Manga> {
    arr.as_array()
        .map(|v| v.iter().filter_map(manga_from_value).collect())
        .unwrap_or_default()
}

impl Source for ToonTop {
    fn new() -> Self {
        Self
    }

    fn get_search_manga_list(
        &self,
        query: Option<String>,
        page: i32,
        filters: Vec<FilterValue>,
    ) -> Result<MangaPageResult> {
        let mut sort = "updated_at";
        let mut genre: Option<String> = None;

        for filter in &filters {
            match filter {
                FilterValue::Sort { index, .. } => {
                    sort = match index {
                        0 => "updated_at",
                        1 => "views_today",
                        2 => "views_week",
                        3 => "rating",
                        4 => "added_at",
                        _ => "updated_at",
                    };
                }
                FilterValue::Select { id, value } if id == "genre" && !value.is_empty() => {
                    genre = Some(
                        value
                            .to_ascii_lowercase()
                            .replace(' ', "-")
                            .replace("---", "-"),
                    );
                }
                _ => {}
            }
        }

        let has_query = query.as_deref().map(|q| !q.is_empty()).unwrap_or(false);

        if has_query {
            let mut qs = QueryParameters::new();
            qs.push("q", query.as_deref());
            qs.push("page", Some(&page.to_string()));
            if let Some(ref g) = genre {
                qs.push("genre", Some(g));
            }
            let html = get(&format!("{BASE_URL}/search?{qs}"))?;
            let data = next_data(&html)?;
            let pp = &data["props"]["pageProps"];
            let entries = items_from_array(&pp["ssrItems"]);
            let has_next_page = entries.len() >= 24;
            Ok(MangaPageResult {
                entries,
                has_next_page,
            })
        } else {
            let mut qs = QueryParameters::new();
            qs.push("page", Some(&page.to_string()));
            qs.push("sort", Some(sort));
            if let Some(ref g) = genre {
                qs.push("genre", Some(g));
            }
            let html = get(&format!("{BASE_URL}/latest?{qs}"))?;
            let data = next_data(&html)?;
            let pp = &data["props"]["pageProps"];
            let entries = items_from_array(&pp["items"]);
            let has_next_page = pp["pagination"]["has_next"].as_bool().unwrap_or(false);
            Ok(MangaPageResult {
                entries,
                has_next_page,
            })
        }
    }

    fn get_manga_update(
        &self,
        mut manga: Manga,
        needs_details: bool,
        needs_chapters: bool,
    ) -> Result<Manga> {
        if !needs_details && !needs_chapters {
            return Ok(manga);
        }

        let page_url = format!("{BASE_URL}{}", manga.key);
        let html = get(&page_url)?;
        let data = next_data(&html)?;
        let m = &data["props"]["pageProps"]["initialManga"];

        if needs_details {
            if let Some(name) = m["name"].as_str() {
                manga.title = name.to_string();
            }
            manga.description = m["summary"].as_str().map(String::from);
            manga.cover = m["cover"].as_str().map(String::from);
            manga.url = Some(page_url);
            manga.viewer = Viewer::Webtoon;

            manga.status = match m["status"].as_str().unwrap_or_default() {
                "ONGOING" => MangaStatus::Ongoing,
                "COMPLETED" => MangaStatus::Completed,
                "HIATUS" => MangaStatus::Hiatus,
                "CANCELLED" | "DROPPED" => MangaStatus::Cancelled,
                _ => MangaStatus::Unknown,
            };

            manga.tags = m["genres"].as_array().map(|arr| {
                arr.iter()
                    .filter_map(|g| g["name"].as_str().map(String::from))
                    .collect()
            });

            manga.authors = m["authors"].as_array().map(|arr| {
                arr.iter()
                    .filter_map(|a| a["name"].as_str().map(String::from))
                    .collect()
            });

            manga.artists = m["artists"].as_array().map(|arr| {
                arr.iter()
                    .filter_map(|a| a["name"].as_str().map(String::from))
                    .collect()
            });
        }

        if needs_chapters {
            let manga_id = m["id"].as_str().unwrap_or_default();
            let chapters_json: Value =
                Request::get(format!("{API_URL}/titles/{manga_id}/chapters"))?
                    .header("User-Agent", USER_AGENT)
                    .json_owned()?;
            manga.chapters = chapters_json["data"]["chapters"]
                .as_array()
                .map(|arr| arr.iter().filter_map(parse_chapter).collect());
        }

        Ok(manga)
    }

    fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
        let url = format!("{BASE_URL}{}{}", manga.key, chapter.key);
        let html = get(&url)?;
        let data = next_data(&html)?;
        let pages_arr = data["props"]["pageProps"]["initialChapter"]["pages"]
            .as_array()
            .ok_or_else(|| error!("Missing chapter pages"))?;
        Ok(pages_arr
            .iter()
            .filter_map(|p| {
                Some(Page {
                    content: PageContent::url(p["url"].as_str()?.to_string()),
                    ..Default::default()
                })
            })
            .collect())
    }
}

impl ListingProvider for ToonTop {
    fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
        let p = page.to_string();
        match listing.name.as_str() {
            "Popular" => {
                let html = get(&format!("{BASE_URL}/popular?page={p}"))?;
                let data = next_data(&html)?;
                let pp = &data["props"]["pageProps"];
                let entries = items_from_array(&pp["items"]);
                let has_next_page = pp["pagination"]["has_next"].as_bool().unwrap_or(false);
                Ok(MangaPageResult {
                    entries,
                    has_next_page,
                })
            }
            "Latest" => {
                let html = get(&format!("{BASE_URL}/latest?page={p}"))?;
                let data = next_data(&html)?;
                let pp = &data["props"]["pageProps"];
                let entries = items_from_array(&pp["items"]);
                let has_next_page = pp["pagination"]["has_next"].as_bool().unwrap_or(false);
                Ok(MangaPageResult {
                    entries,
                    has_next_page,
                })
            }
            "Uncensored" => {
                let html = get(&format!("{BASE_URL}/search?uncensored=true&page={p}"))?;
                let data = next_data(&html)?;
                let pp = &data["props"]["pageProps"];
                let entries = items_from_array(&pp["ssrItems"]);
                let has_next_page = pp["ssrPagination"]["has_next"]
                    .as_bool()
                    .unwrap_or(entries.len() >= 24);
                Ok(MangaPageResult {
                    entries,
                    has_next_page,
                })
            }
            "Raw" => {
                let html = get(&format!("{BASE_URL}/search?raw=true&page={p}"))?;
                let data = next_data(&html)?;
                let pp = &data["props"]["pageProps"];
                let entries = items_from_array(&pp["ssrItems"]);
                let has_next_page = pp["ssrPagination"]["has_next"]
                    .as_bool()
                    .unwrap_or(entries.len() >= 24);
                Ok(MangaPageResult {
                    entries,
                    has_next_page,
                })
            }
            tab => {
                let tab_id = match tab {
                    "Top Today" => "top-day",
                    "Top This Week" => "top-week",
                    "Top This Month" => "top-month",
                    "Most Views" => "most-views",
                    "Most Bookmarks" => "most-bookmarks",
                    _ => "top-day",
                };
                let html = get(&format!("{BASE_URL}/ranking?tab={tab_id}&page={p}"))?;
                let data = next_data(&html)?;
                let pp = &data["props"]["pageProps"];
                let entries = items_from_array(&pp["initialItems"]);
                let has_next_page = pp["initialPagination"]["has_next"]
                    .as_bool()
                    .unwrap_or(false);
                Ok(MangaPageResult {
                    entries,
                    has_next_page,
                })
            }
        }
    }
}

impl Home for ToonTop {
    fn get_home(&self) -> Result<HomeLayout> {
        let html = get(&format!("{BASE_URL}/home"))?;
        let data = next_data(&html)?;
        let pp = &data["props"]["pageProps"];

        let listing = |id: &str, name: &str| Listing {
            id: id.into(),
            name: name.into(),
            kind: ListingKind::Default,
        };

        let trending = pp["trendingItems"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| manga_from_value(item).map(Link::from))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let popular = pp["popularItems"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| manga_from_value(item).map(Link::from))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let latest = pp["topUpdateItems"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(manga_with_chapter_from_value)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(HomeLayout {
            components: vec![
                HomeComponent {
                    title: Some("Trending".into()),
                    subtitle: None,
                    value: HomeComponentValue::Scroller {
                        entries: trending,
                        listing: Some(listing("Top Today", "Top Today")),
                    },
                },
                HomeComponent {
                    title: Some("Popular".into()),
                    subtitle: None,
                    value: HomeComponentValue::MangaList {
                        ranking: true,
                        page_size: Some(10),
                        entries: popular,
                        listing: Some(listing("Popular", "Popular")),
                    },
                },
                HomeComponent {
                    title: Some("Latest Updates".into()),
                    subtitle: None,
                    value: HomeComponentValue::MangaChapterList {
                        page_size: Some(12),
                        entries: latest,
                        listing: Some(listing("Latest", "Latest")),
                    },
                },
            ],
        })
    }
}

impl DeepLinkHandler for ToonTop {
    fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
        if !url.starts_with(BASE_URL) {
            return Ok(None);
        }
        let path = &url[BASE_URL.len()..];
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        match parts.len() {
            1 => Ok(Some(DeepLinkResult::Manga {
                key: format!("/{}", parts[0]),
            })),
            2 => Ok(Some(DeepLinkResult::Chapter {
                manga_key: format!("/{}", parts[0]),
                key: format!("/{}", parts[1]),
            })),
            _ => Ok(None),
        }
    }
}

impl ImageRequestProvider for ToonTop {
    fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
        Ok(Request::get(url)?
            .header("Referer", &format!("{BASE_URL}/"))
            .header("User-Agent", USER_AGENT))
    }
}

register_source!(
    ToonTop,
    ListingProvider,
    Home,
    DeepLinkHandler,
    ImageRequestProvider
);
