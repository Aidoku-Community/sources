use aidoku::{
    Chapter, ContentRating, DeepLinkResult, FilterValue, Listing, Manga, MangaPageResult,
    Page, PageContent, PageContext, Result,
    alloc::{
        collections::BTreeMap,
        format,
        string::String,
        vec::Vec,
    },
    imports::{net::Request, std::current_date},
};
use core::cmp::Reverse;

use crate::{AllSeriesItem, SeriesCache, SeriesDetail, strip_html};
use super::{PAGE_SIZE, Params, SERIES_TTL};

pub trait Impl {
    fn new() -> Self;
    fn params(&self) -> Params;

    fn content_rating_for(&self, _det: &SeriesDetail) -> ContentRating {
        ContentRating::Safe
    }

    fn api_get(&self, url: &str) -> Result<Request> {
        Ok(Request::get(url)?
            .header("Accept", "application/json")
            .header("User-Agent", "Aidoku"))
    }

    fn html_get(&self, url: &str) -> Result<Request> {
        Ok(Request::get(url)?.header("User-Agent", "Aidoku"))
    }

    fn fetch_all_series(&self, params: &Params, cache: &mut SeriesCache) -> Result<Vec<(String, AllSeriesItem)>> {
        let now = current_date();
        if let Some((ref data, ts)) = *cache
            && now - ts < SERIES_TTL
        {
            return Ok(data.clone());
        }
        let map: BTreeMap<String, AllSeriesItem> =
            self.api_get(&format!("{}/api/get_all_series/", params.base_url))?.json_owned()?;
        let data: Vec<(String, AllSeriesItem)> =
            map.into_iter().filter(|(_, v)| !v.slug.is_empty()).collect();
        *cache = Some((data.clone(), now));
        Ok(data)
    }

    // The series/oneshots/nsfw listing pages inject cards via a JS `series_data` array into
    // an empty <div>. Static HTML parsing finds nothing; raw text splitting extracts the slugs.
    fn fetch_html_series_list(&self, params: &Params, path: &str, page: i32, cache: &mut SeriesCache) -> Result<MangaPageResult> {
        let base = params.base_url;
        let text = self.html_get(&format!("{base}{path}"))?.string()?;

        let mut series_map: BTreeMap<String, (String, Option<String>)> = self
            .fetch_all_series(params, cache)?
            .into_iter()
            .map(|(title, item)| {
                let cover = if item.cover.is_empty() {
                    None
                } else {
                    Some(format!("{base}{}", item.cover))
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
                let url = format!("{base}/read/manga/{slug}/");
                let (title, cover) = if let Some(entry) = series_map.remove(&slug) {
                    entry
                } else {
                    (slug.clone(), None)
                };
                Manga { key: slug, title, url: Some(url), cover, ..Default::default() }
            })
            .collect();

        let start = ((page - 1) as usize) * PAGE_SIZE;
        let has_next_page = start + PAGE_SIZE < all_entries.len();
        let entries = all_entries.into_iter().skip(start).take(PAGE_SIZE).collect();
        Ok(MangaPageResult { entries, has_next_page })
    }

    fn fetch_latest_chapters_list(&self, params: &Params, page: i32, cache: &mut SeriesCache) -> Result<MangaPageResult> {
        let base = params.base_url;
        let html = self.html_get(&format!("{base}/latest_chapters/"))?.html()?;

        let mut series_map: BTreeMap<String, (String, Option<String>)> = self
            .fetch_all_series(params, cache)?
            .into_iter()
            .map(|(title, item)| {
                let cover = if item.cover.is_empty() {
                    None
                } else {
                    Some(format!("{base}{}", item.cover))
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
                    let url = format!("{base}/read/manga/{slug}/");
                    let (title, cover) = if let Some(entry) = series_map.remove(&slug) {
                        entry
                    } else {
                        let t = row
                            .select_first("td.chapter-title a")
                            .and_then(|a| a.text())
                            .unwrap_or_else(|| slug.clone());
                        (t, None)
                    };
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

    fn get_search_manga_list(
        &self,
        params: &Params,
        query: Option<String>,
        page: i32,
        filters: Vec<FilterValue>,
        cache: &mut SeriesCache,
    ) -> Result<MangaPageResult> {
        let mut all = self.fetch_all_series(params, cache)?;

        let sort_latest = filters.iter().any(|f| {
            matches!(f, FilterValue::Sort { index, .. } if *index == 1)
        });

        if let Some(q) = &query {
            let q_lower = q.to_lowercase();
            all.retain(|(title, _)| title.to_lowercase().contains(&q_lower));
        }

        if sort_latest {
            all.sort_by_key(|(_, item)| Reverse(item.last_updated));
        } else {
            all.sort_by(|a, b| a.0.cmp(&b.0));
        }

        let start = ((page - 1) as usize) * PAGE_SIZE;
        let has_next_page = start + PAGE_SIZE < all.len();
        let entries = all
            .into_iter()
            .skip(start)
            .take(PAGE_SIZE)
            .map(|(title, item)| item.into_manga(title, params.base_url))
            .collect();

        Ok(MangaPageResult { entries, has_next_page })
    }

    fn get_manga_update(
        &self,
        params: &Params,
        mut manga: Manga,
        needs_details: bool,
        needs_chapters: bool,
    ) -> Result<Manga> {
        let base = params.base_url;
        let mut det: SeriesDetail =
            self.api_get(&format!("{base}/api/series/{}/", manga.key))?.json_owned()?;

        if needs_details {
            manga.title = String::from(det.title.trim());
            manga.cover = if det.cover.is_empty() {
                None
            } else {
                Some(format!("{base}{}", det.cover))
            };
            manga.url = Some(format!("{base}/read/manga/{}/", det.slug));
            manga.content_rating = self.content_rating_for(&det);
            manga.viewer = params.viewer;

            let desc = strip_html(&det.description);
            if !desc.is_empty() {
                manga.description = Some(desc);
            }
            let author = core::mem::take(&mut det.author);
            let artist = core::mem::take(&mut det.artist);
            let has_artist = !artist.is_empty() && artist != author;
            if !author.is_empty() { manga.authors = Some(vec![author]); }
            if has_artist { manga.artists = Some(vec![artist]); }
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

    fn get_page_list(&self, params: &Params, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
        let base = params.base_url;
        let det: SeriesDetail =
            self.api_get(&format!("{base}/api/series/{}/", manga.key))?.json_owned()?;

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

    fn get_manga_list(&self, params: &Params, listing: Listing, page: i32, cache: &mut SeriesCache) -> Result<MangaPageResult> {
        let base = params.base_url;
        match listing.id.as_str() {
            "Series" => return self.fetch_html_series_list(params, "/series/", page, cache),
            "Oneshots" => return self.fetch_html_series_list(params, "/oneshots/", page, cache),
            "NSFW" => return self.fetch_html_series_list(params, "/nsfw/", page, cache),
            "Latest Chapters" => return self.fetch_latest_chapters_list(params, page, cache),
            _ => {}
        }

        let all = self.fetch_all_series(params, cache)?;
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

    fn handle_deep_link(&self, params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
        let base = params.base_url;
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

    fn get_image_request(&self, _params: &Params, url: String, _context: Option<PageContext>) -> Result<Request> {
        self.api_get(&url)
    }
}
