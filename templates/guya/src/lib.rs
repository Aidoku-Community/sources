#![no_std]

use aidoku::{
    Chapter, ContentRating, DeepLinkHandler, DeepLinkResult,
    FilterValue, Home, HomeComponent, HomeComponentValue, HomeLayout,
    ImageRequestProvider, Link, Listing, ListingKind, ListingProvider, Manga, MangaPageResult,
    Page, PageContent, PageContext, Result, Source, Viewer,
    alloc::{
        collections::BTreeMap,
        format,
        string::String,
        vec,
        vec::Vec,
    },
    imports::net::Request,
};

mod helpers;
mod models;

pub use helpers::strip_html;
pub use models::*;

const PAGE_SIZE: usize = 20;

const USER_AGENT: &str = "aidoku/1.0 CFNetwork/1490.0.4 Darwin/23.4.0";

pub struct Params {
    pub base_url: &'static str,
    pub viewer: Viewer,
}

pub trait Impl {
    fn new() -> Self;
    fn params(&self) -> Params;
    fn content_rating_for(&self, _det: &SeriesDetail) -> ContentRating {
        ContentRating::Safe
    }
}

pub struct Guya<T: Impl> {
    _inner: T,
    params: Params,
}

fn api_get(url: &str) -> Result<Request> {
    Ok(Request::get(url)?
        .header("Accept", "application/json")
        .header("User-Agent", USER_AGENT))
}

fn html_get(url: &str) -> Result<Request> {
    Ok(Request::get(url)?
        .header("User-Agent", USER_AGENT))
}

fn fetch_all_series(base_url: &str) -> Result<Vec<(String, AllSeriesItem)>> {
    let map: BTreeMap<String, AllSeriesItem> =
        api_get(&format!("{base_url}/api/get_all_series/"))?.json_owned()?;
    Ok(map.into_iter().filter(|(_, v)| !v.slug.is_empty()).collect())
}

// The series/oneshots/nsfw listing pages inject cards via a JS `series_data` array into
// an empty <div>. Static HTML parsing finds nothing; raw text splitting extracts the slugs.
fn fetch_html_series_list(base_url: &str, path: &str, page: i32) -> Result<MangaPageResult> {
    let text = html_get(&format!("{base_url}{path}"))?.string()?;

    let series_map: BTreeMap<String, (String, Option<String>)> = fetch_all_series(base_url)?
        .into_iter()
        .map(|(title, item)| {
            let cover = if item.cover.is_empty() {
                None
            } else {
                Some(format!("{base_url}{}", item.cover))
            };
            (item.slug, (title, cover))
        })
        .collect();

    let mut slugs: Vec<String> = Vec::new();
    for part in text.split("href=\"/read/manga/").skip(1) {
        let slug = part.split('/').next().unwrap_or("");
        if !slug.is_empty() && !slugs.iter().any(|s| s == slug) {
            slugs.push(String::from(slug));
        }
    }

    let all_entries: Vec<Manga> = slugs
        .into_iter()
        .map(|slug| {
            let url = format!("{base_url}/read/manga/{slug}/");
            let (title, cover) = series_map
                .get(&slug)
                .map(|(t, c)| (t.clone(), c.clone()))
                .unwrap_or_else(|| (slug.clone(), None));
            Manga { key: slug, title, url: Some(url), cover, ..Default::default() }
        })
        .collect();

    let start = ((page - 1) as usize) * PAGE_SIZE;
    let has_next_page = start + PAGE_SIZE < all_entries.len();
    let entries = all_entries.into_iter().skip(start).take(PAGE_SIZE).collect();
    Ok(MangaPageResult { entries, has_next_page })
}

fn fetch_latest_chapters_list(base_url: &str, page: i32) -> Result<MangaPageResult> {
    let html = html_get(&format!("{base_url}/latest_chapters/"))?.html()?;

    let series_map: BTreeMap<String, (String, Option<String>)> = fetch_all_series(base_url)?
        .into_iter()
        .map(|(title, item)| {
            let cover = if item.cover.is_empty() {
                None
            } else {
                Some(format!("{base_url}{}", item.cover))
            };
            (item.slug, (title, cover))
        })
        .collect();

    let mut all_entries: Vec<Manga> = Vec::new();

    if let Some(rows) = html.select("tr[data-serie]") {
        for row in rows {
            if let Some(slug) = row.attr("data-serie") {
                if all_entries.iter().any(|m| m.key == slug) {
                    continue;
                }
                let url = format!("{base_url}/read/manga/{slug}/");
                let (title, cover) = series_map
                    .get(&slug)
                    .map(|(t, c)| (t.clone(), c.clone()))
                    .unwrap_or_else(|| {
                        let t = row
                            .select_first("td.chapter-title a")
                            .and_then(|a| a.text())
                            .unwrap_or_else(|| slug.clone());
                        (t, None)
                    });
                all_entries.push(Manga {
                    key: slug,
                    title,
                    url: Some(url),
                    cover,
                    ..Default::default()
                });
            }
        }
    }

    let start = ((page - 1) as usize) * PAGE_SIZE;
    let has_next_page = start + PAGE_SIZE < all_entries.len();
    let entries = all_entries.into_iter().skip(start).take(PAGE_SIZE).collect();
    Ok(MangaPageResult { entries, has_next_page })
}

impl<T: Impl> Source for Guya<T> {
    fn new() -> Self {
        let inner = T::new();
        let params = inner.params();
        Self { _inner: inner, params }
    }

    fn get_search_manga_list(
        &self,
        query: Option<String>,
        page: i32,
        filters: Vec<FilterValue>,
    ) -> Result<MangaPageResult> {
        let mut all = fetch_all_series(self.params.base_url)?;

        let sort_latest = filters.iter().any(|f| {
            matches!(f, FilterValue::Select { id, value } if id == "sort" && value == "latest")
        });

        if let Some(q) = &query {
            let q_lower = q.to_lowercase();
            all.retain(|(title, _)| title.to_lowercase().contains(&q_lower));
        }

        if sort_latest {
            all.sort_by_key(|(_, item)| core::cmp::Reverse(item.last_updated));
        } else {
            all.sort_by(|a, b| a.0.cmp(&b.0));
        }

        let start = ((page - 1) as usize) * PAGE_SIZE;
        let has_next_page = start + PAGE_SIZE < all.len();
        let entries = all
            .into_iter()
            .skip(start)
            .take(PAGE_SIZE)
            .map(|(title, item)| item.into_manga(title, self.params.base_url))
            .collect();

        Ok(MangaPageResult { entries, has_next_page })
    }

    fn get_manga_update(
        &self,
        mut manga: Manga,
        needs_details: bool,
        needs_chapters: bool,
    ) -> Result<Manga> {
        let base = self.params.base_url;
        let mut det: SeriesDetail =
            api_get(&format!("{base}/api/series/{}/", manga.key))?.json_owned()?;

        if needs_details {
            manga.title = String::from(det.title.trim());
            manga.cover = if det.cover.is_empty() {
                None
            } else {
                Some(format!("{base}{}", det.cover))
            };
            manga.url = Some(format!("{base}/read/manga/{}/", det.slug));
            manga.content_rating = self._inner.content_rating_for(&det);
            manga.viewer = self.params.viewer;

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
            let mut chapters: Vec<Chapter> = core::mem::take(&mut det.chapters.0)
                .into_iter()
                .filter(|(_, ch)| ch.is_public)
                .map(|(num_str, ch)| {
                    let chapter_number = num_str.parse().ok();
                    let url = format!("{base}/read/manga/{}/{}/1/", det.slug, num_str);
                    let scanlators: Vec<String> = ch
                        .groups
                        .group_ids()
                        .filter_map(|gid| det.groups.get(gid))
                        .map(String::from)
                        .collect();
                    Chapter {
                        key: num_str,
                        chapter_number,
                        title: ch.title.filter(|t| !t.is_empty()),
                        date_uploaded: ch.release_date.0,
                        scanlators: if scanlators.is_empty() { None } else { Some(scanlators) },
                        url: Some(url),
                        ..Default::default()
                    }
                })
                .collect();
            chapters.sort_by(|a, b| {
                b.chapter_number
                    .unwrap_or(0.0)
                    .partial_cmp(&a.chapter_number.unwrap_or(0.0))
                    .unwrap_or(core::cmp::Ordering::Equal)
            });
            manga.chapters = Some(chapters);
        }

        Ok(manga)
    }

    fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
        let base = self.params.base_url;
        let det: SeriesDetail =
            api_get(&format!("{base}/api/series/{}/", manga.key))?.json_owned()?;

        let ch = match det.chapters.find(&chapter.key) {
            Some(ch) => ch,
            None => return Ok(Vec::new()),
        };

        let pages = match ch.groups.0.first() {
            Some((group_id, filenames)) => filenames
                .iter()
                .map(|filename| Page {
                    content: PageContent::url(format!(
                        "{base}/media/manga/{}/chapters/{}/{}/{}",
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

impl<T: Impl> ListingProvider for Guya<T> {
    fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
        let base = self.params.base_url;
        match listing.id.as_str() {
            "Series"          => return fetch_html_series_list(base, "/series/", page),
            "Oneshot"         => return fetch_html_series_list(base, "/oneshots/", page),
            "NSFW"            => return fetch_html_series_list(base, "/nsfw/", page),
            "Latest Chapter"  => return fetch_latest_chapters_list(base, page),
            _ => {}
        }

        let mut all = fetch_all_series(base)?;
        match listing.id.as_str() {
            "Latest" => all.sort_by_key(|(_, item)| core::cmp::Reverse(item.last_updated)),
            _        => all.sort_by(|a, b| a.0.cmp(&b.0)),
        }

        let start = ((page - 1) as usize) * PAGE_SIZE;
        let has_next_page = start + PAGE_SIZE < all.len();
        let entries = all
            .into_iter()
            .skip(start)
            .take(PAGE_SIZE)
            .map(|(title, item)| item.into_manga(title, base))
            .collect();
        Ok(MangaPageResult { entries, has_next_page })
    }
}

impl<T: Impl> Home for Guya<T> {
    fn get_home(&self) -> Result<HomeLayout> {
        let mut all = fetch_all_series(self.params.base_url)?;
        all.sort_by_key(|(_, item)| core::cmp::Reverse(item.last_updated));

        let entries: Vec<Link> = all
            .into_iter()
            .take(20)
            .map(|(title, item)| item.into_manga(title, self.params.base_url).into())
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

impl<T: Impl> DeepLinkHandler for Guya<T> {
    fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
        let base = self.params.base_url;
        let Some(path) = url.strip_prefix(base) else {
            return Ok(None);
        };
        let rest = path
            .strip_prefix("/read/manga/")
            .or_else(|| path.strip_prefix("/reader/manga/"))
            .or_else(|| path.strip_prefix("/reader/series/"))
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

impl<T: Impl> ImageRequestProvider for Guya<T> {
    fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
        api_get(&url)
    }
}
