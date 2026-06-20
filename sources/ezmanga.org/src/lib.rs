#![no_std]

#[no_mangle]
pub unsafe extern "C" fn memcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    for i in 0..n {
        let a = *s1.add(i);
        let b = *s2.add(i);
        if a != b {
            return a as i32 - b as i32;
        }
    }
    0
}

use aidoku::{
    Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, DynamicFilters,
    DynamicListings, Filter, FilterValue, Home, HomeComponent, HomeComponentValue, HomeLayout,
    Listing, ListingKind, ListingProvider, Link, Manga, MangaPageResult, MangaStatus,
    Page, PageContent, Result, SelectFilter, Source, Viewer,
    alloc::{
        borrow::Cow,
        format,
        string::String,
        vec,
        vec::Vec,
    },
    helpers::uri::encode_uri_component,
    imports::net::Request,
    prelude::*,
};
use serde::Deserialize;

const API_BASE: &str = "https://vapi.ezmanga.org/api/v1";
const BASE_URL: &str = "https://ezmanga.org";
const UA: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1";

fn api_get(url: &str) -> Result<Request> {
    Ok(Request::get(url)?
        .header("User-Agent", UA)
        .header("Origin", BASE_URL)
        .header("Referer", "https://ezmanga.org/"))
}

// --- API response types ---

#[derive(Deserialize)]
struct ApiList<T> {
    data: Vec<T>,
    next: Option<i32>,
}

#[derive(Deserialize)]
struct ApiSeriesItem {
    slug: String,
    title: String,
    cover: String,
}

#[derive(Deserialize)]
struct ApiSeriesDetail {
    slug: String,
    title: String,
    description: Option<String>,
    author: Option<String>,
    artist: Option<String>,
    cover: String,
    status: Option<String>,
    genres: Option<Vec<ApiGenre>>,
}

#[derive(Deserialize)]
struct ApiGenre {
    name: String,
}

#[derive(Deserialize)]
struct ApiChapter {
    slug: String,
    number: f64,
    title: Option<String>,
    #[serde(rename = "createdAt")]
    created_at: Option<String>,
    #[serde(default)]
    locked: bool,
}

#[derive(Deserialize)]
struct ApiChapterDetail {
    images: Vec<ApiImage>,
}

#[derive(Deserialize)]
struct ApiImage {
    url: String,
}

// --- Helpers ---

fn parse_status(s: Option<&str>) -> MangaStatus {
    match s {
        Some("ONGOING") => MangaStatus::Ongoing,
        Some("COMPLETED") => MangaStatus::Completed,
        Some("DROPPED") | Some("CANCELLED") => MangaStatus::Cancelled,
        Some("HIATUS") => MangaStatus::Hiatus,
        _ => MangaStatus::Unknown,
    }
}

fn strip_html(html: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for ch in html.chars() {
        match ch {
            '<' => depth += 1,
            '>' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    String::from(out.trim())
}

fn parse_num_bytes(bytes: &[u8]) -> Option<i64> {
    let mut n = 0i64;
    for &b in bytes {
        if !b.is_ascii_digit() {
            return None;
        }
        n = n * 10 + (b - b'0') as i64;
    }
    Some(n)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let m = if m <= 2 { m + 9 } else { m - 3 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * m + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn parse_date(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 10 {
        return None;
    }
    let y = parse_num_bytes(&b[0..4])?;
    let mo = parse_num_bytes(&b[5..7])?;
    let d = parse_num_bytes(&b[8..10])?;
    let days = days_from_civil(y, mo, d);
    let secs = if b.len() >= 19 {
        let h = parse_num_bytes(&b[11..13])?;
        let mi = parse_num_bytes(&b[14..16])?;
        let se = parse_num_bytes(&b[17..19])?;
        h * 3600 + mi * 60 + se
    } else {
        0
    };
    Some(days * 86400 + secs)
}

fn item_to_manga(s: ApiSeriesItem) -> Manga {
    Manga {
        url: Some(format!("{}/series/{}", BASE_URL, s.slug)),
        key: s.slug,
        title: String::from(s.title.trim()),
        cover: if s.cover.is_empty() { None } else { Some(s.cover) },
        ..Default::default()
    }
}

fn item_to_link(s: ApiSeriesItem) -> Link {
    let manga = item_to_manga(s);
    Link::from(manga)
}

fn build_browse_url(page: i32, filters: &[FilterValue]) -> String {
    let mut url = format!("{}/series?page={}", API_BASE, page);
    for filter in filters {
        if let FilterValue::Select { id, value } = filter {
            match id.as_str() {
                "sort" => match value.as_str() {
                    "Popular" => url.push_str("&sort=popular"),
                    "Latest" => url.push_str("&sort=latest"),
                    _ => {}
                },
                "status" => match value.as_str() {
                    "Ongoing" => url.push_str("&status=ONGOING"),
                    "Completed" => url.push_str("&status=COMPLETED"),
                    "Dropped" => url.push_str("&status=DROPPED"),
                    "Hiatus" => url.push_str("&status=HIATUS"),
                    _ => {}
                },
                "type" => match value.as_str() {
                    "Manhwa" => url.push_str("&type=MANHWA"),
                    "Manga" => url.push_str("&type=MANGA"),
                    "Manhua" => url.push_str("&type=MANHUA"),
                    "Novel" => url.push_str("&type=NOVEL"),
                    _ => {}
                },
                _ => {}
            }
        }
    }
    url
}

// --- Source ---

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
            None => build_browse_url(page, &filters),
        };

        let resp: ApiList<ApiSeriesItem> = api_get(&url)?.json_owned()?;
        let has_next = resp.next.is_some();
        let entries = resp.data.into_iter().map(item_to_manga).collect();

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
        let entries = resp.data.into_iter().map(item_to_manga).collect();

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
        let popular_links: Vec<Link> = popular_resp.data.into_iter().map(item_to_link).collect();

        let latest_resp: ApiList<ApiSeriesItem> =
            api_get(&format!("{}/series?page=1&sort=latest", API_BASE))?.json_owned()?;
        let latest_links: Vec<Link> = latest_resp.data.into_iter().map(item_to_link).collect();

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
        // https://ezmanga.org/series/<slug>
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
