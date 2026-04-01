#![no_std]
use aidoku::{
    Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterItem, FilterValue, Home,
    HomeComponent, HomeComponentValue, HomeLayout, ImageRequestProvider, Link, LinkValue, Listing,
    ListingKind, ListingProvider, Manga, MangaPageResult, MangaStatus, MangaWithChapter, Page,
    PageContent, PageContext, Result, Source, Viewer,
    alloc::{String, Vec, format, vec},
    imports::{html::Element, net::Request, std::current_date},
    prelude::*,
};

const BASE_URL: &str = "https://aquareader.net";

struct AquaReader;

fn make_listing(id: &str, name: &str) -> Listing {
    Listing {
        id: String::from(id),
        name: String::from(name),
        kind: ListingKind::Default,
    }
}

fn build_listing_url(name: &str, page: i32) -> String {
    let orderby: Option<&str> = match name {
        "Popular" => Some("views"),
        "New Releases" => Some("new-manga"),
        "Trending" => Some("trending"),
        "Latest Completed" => None,
        _ => None,
    };

    if name == "Latest Completed" {
        return if page <= 1 {
            format!(
                "{}/page/1/?s&post_type=wp-manga&status[]=end&m_orderby=modified",
                BASE_URL
            )
        } else {
            format!(
                "{}/page/{}/?s&post_type=wp-manga&status[]=end&m_orderby=modified",
                BASE_URL, page
            )
        };
    }

    if let Some(order) = orderby {
        return if page <= 1 {
            format!("{}/manga/?m_orderby={}", BASE_URL, order)
        } else {
            format!("{}/manga/page/{}/?m_orderby={}", BASE_URL, page, order)
        };
    }
    if page <= 1 {
        format!("{}/", BASE_URL)
    } else {
        format!("{}/page/{}/", BASE_URL, page)
    }
}
fn parse_manga_list(url: &str) -> Result<MangaPageResult> {
    let html = Request::get(url)?.html()?;
    let mut entries: Vec<Manga> = Vec::new();
    if let Some(items) = html.select(".c-tabs-item__content") {
        for item in items {
            let key = item
                .select_first(".tab-thumb a")
                .and_then(|a: Element| a.attr("href"))
                .map(|s: String| String::from(s.trim_end_matches('#')))
                .unwrap_or_default();
            let title = item
                .select_first(".post-title a")
                .and_then(|el: Element| el.text())
                .unwrap_or_default();
            let cover = item
                .select_first(".tab-thumb img")
                .and_then(|img: Element| img.attr("data-src").or_else(|| img.attr("abs:src")));
            if !key.is_empty() && !title.is_empty() {
                entries.push(Manga {
                    key,
                    title,
                    cover,
                    ..Default::default()
                });
            }
        }
    }
    if entries.is_empty() {
        if let Some(items) = html.select(".page-item-detail") {
            for item in items {
                let key = item
                    .select_first("a")
                    .and_then(|a: Element| a.attr("href"))
                    .map(|s: String| String::from(s.trim_end_matches('#')))
                    .unwrap_or_default();
                let title = item
                    .select_first(".post-title")
                    .and_then(|el: Element| el.text())
                    .unwrap_or_default();
                let cover = item
                    .select_first("img")
                    .and_then(|img: Element| img.attr("data-src").or_else(|| img.attr("abs:src")));
                if !key.is_empty() && !title.is_empty() {
                    entries.push(Manga {
                        key,
                        title,
                        cover,
                        ..Default::default()
                    });
                }
            }
        }
    }

    let has_next_page = html.select_first("a[class*='next']").is_some();
    Ok(MangaPageResult {
        entries,
        has_next_page,
    })
}
fn parse_search_list(url: &str) -> Result<MangaPageResult> {
    parse_manga_list(url)
}
fn relative_date_to_timestamp(text: &str) -> Option<i64> {
    let t = text.trim();
    if !t.contains("ago") {
        return None;
    }
    let parts: Vec<&str> = t.split_whitespace().collect();
    if parts.len() < 3 {
        return None;
    }
    let n: i64 = parts[0].parse().ok()?;
    let unit = parts[1];
    let secs: i64 = if unit.starts_with("min") {
        60
    } else if unit.starts_with("hour") {
        3600
    } else if unit.starts_with("day") {
        86400
    } else if unit.starts_with("week") {
        604800
    } else if unit.starts_with("month") {
        2592000
    } else if unit.starts_with("year") {
        31536000
    } else {
        return None;
    };
    Some(current_date() - n * secs)
}
fn get_status(html: &aidoku::imports::html::Document) -> MangaStatus {
    if let Some(items) = html.select(".post-content_item") {
        for item in items {
            let heading = item
                .select_first(".summary-heading h5")
                .and_then(|h: Element| h.text())
                .unwrap_or_default();
            if heading.trim() == "Status" {
                if let Some(text) = item
                    .select_first(".summary-content")
                    .and_then(|el: Element| el.text())
                {
                    return match text.trim() {
                        "OnGoing" | "Ongoing" | "ongoing" | "Serialization" => MangaStatus::Ongoing,
                        "Completed" | "completed" => MangaStatus::Completed,
                        "Cancelled" | "cancelled" | "Dropped" => MangaStatus::Cancelled,
                        "Hiatus" | "hiatus" | "On Hold" => MangaStatus::Hiatus,
                        _ => MangaStatus::Unknown,
                    };
                }
            }
        }
    }
    MangaStatus::Unknown
}

impl Source for AquaReader {
    fn new() -> Self {
        Self
    }

    fn get_search_manga_list(
        &self,
        _query: Option<String>,
        _page: i32,
        _filters: Vec<FilterValue>,
    ) -> Result<MangaPageResult> {
        let mut sort: Option<String> = None;
        let mut status_param = String::new();
        let mut genre_params = String::new();
        let mut media_type: Option<String> = None;

        for filter in _filters {
            match filter {
                FilterValue::Sort { id, index, .. } if id == "type" => {
                    media_type = match index {
                        1 => Some(String::from("manga")),
                        2 => Some(String::from("manhwa")),
                        3 => Some(String::from("manhua")),
                        _ => None,
                    };
                }

                FilterValue::Sort { id, index, .. } if id == "sort" => {
                    sort = match index {
                        1 => Some(String::from("alphabet")),
                        2 => Some(String::from("new-manga")),
                        3 => Some(String::from("modified")),
                        4 => Some(String::from("rating")),
                        5 => Some(String::from("trending")),
                        _ => None,
                    };
                }

                FilterValue::Select { id, value } if id == "status" => {
                    status_param = match value.as_str() {
                        "Completed" => String::from("&status[]=end"),
                        "Ongoing" => String::from("&status[]=on-going"),
                        "Hiatus" => String::from("&status[]=on-hold"),
                        _ => String::new(),
                    };
                }

                FilterValue::MultiSelect { id, included, .. } if id == "genre" => {
                    for genre in included {
                        let slug = genre_to_slug(genre.as_str());
                        if !slug.is_empty() {
                            genre_params.push_str(&format!("&genre[]={}", slug));
                        }
                    }
                }

                _ => {}
            }
        }
        let orderby_q = match sort {
            Some(ref s) => format!("?m_orderby={}", s),
            None => String::new(),
        };
        let orderby_amp = match sort {
            Some(ref s) => format!("&m_orderby={}", s),
            None => String::new(),
        };

        let query = _query.unwrap_or_default();
        if let Some(ref type_slug) = media_type {
            if status_param.is_empty() && genre_params.is_empty() && query.is_empty() {
                let url = if _page <= 1 {
                    format!("{}/manga-genre/{}/{}", BASE_URL, type_slug, orderby_q)
                } else {
                    format!(
                        "{}/manga-genre/{}/page/{}/{}",
                        BASE_URL, type_slug, _page, orderby_q
                    )
                };
                return parse_manga_list(&url);
            }
            genre_params.push_str(&format!("&genre[]={}", type_slug));
        }
        if query.is_empty() && status_param.is_empty() && genre_params.is_empty() {
            let url = if _page <= 1 {
                format!("{}/manga/{}", BASE_URL, orderby_q)
            } else {
                format!("{}/manga/page/{}/{}", BASE_URL, _page, orderby_q)
            };
            return parse_manga_list(&url);
        }
        let url = format!(
            "{}/page/{}/?s={}&post_type=wp-manga{}{}{}",
            BASE_URL, _page, query, orderby_amp, status_param, genre_params
        );

        parse_search_list(&url)
    }

    fn get_manga_update(
        &self,
        manga: Manga,
        needs_details: bool,
        needs_chapters: bool,
    ) -> Result<Manga> {
        let url = String::from(manga.key.trim_end_matches('#').trim_end_matches('/')) + "/";
        let html = Request::get(&url)?.html()?;
        let mut updated = manga;

        if needs_details {
            if let Some(title) = html
                .select_first(".post-title h1")
                .and_then(|el: Element| el.text())
            {
                updated.title = title;
            }

            updated.cover = html
                .select_first(".summary_image img")
                .and_then(|img: Element| img.attr("data-src").or_else(|| img.attr("abs:src")))
                .or(updated.cover);
            updated.description = html
                .select_first(".summary__content p")
                .and_then(|el: Element| el.text())
                .or_else(|| {
                    html.select_first(".description-summary p")
                        .and_then(|el: Element| el.text())
                });

            updated.authors = html.select(".author-content a").map(|els| {
                els.map(|el: Element| el.text().unwrap_or_default())
                    .filter(|s: &String| !s.is_empty())
                    .collect()
            });

            updated.artists = html.select(".artist-content a").map(|els| {
                els.map(|el: Element| el.text().unwrap_or_default())
                    .filter(|s: &String| !s.is_empty())
                    .collect()
            });

            updated.status = get_status(&html);
            updated.viewer = Viewer::Webtoon;
            let mut all_tags: Vec<String> = html
                .select(".genres-content a")
                .map(|els| {
                    els.map(|el: Element| String::from(el.text().unwrap_or_default().trim()))
                        .filter(|s: &String| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            if let Some(items) = html.select(".post-content_item") {
                for item in items {
                    let heading = item
                        .select_first(".summary-heading h5")
                        .and_then(|h: Element| h.text())
                        .unwrap_or_default();
                    if heading.trim() == "Type" {
                        if let Some(type_text) = item
                            .select_first(".summary-content")
                            .and_then(|el: Element| el.text())
                        {
                            let t = String::from(type_text.trim());
                            if !t.is_empty() && !all_tags.contains(&t) {
                                all_tags.push(t);
                            }
                        }
                        break;
                    }
                }
            }
            if all_tags.iter().any(|t| t.eq_ignore_ascii_case("ecchi")) {
                updated.content_rating = ContentRating::Suggestive;
            }

            updated.tags = if all_tags.is_empty() {
                None
            } else {
                Some(all_tags)
            };
        }

        if needs_chapters {
            let mut chapters: Vec<Chapter> = Vec::new();

            if let Some(items) = html.select("li.wp-manga-chapter") {
                let all: Vec<_> = items.collect();
                for item in all {
                    let anchor = item.select_first("a");
                    let key = anchor
                        .as_ref()
                        .and_then(|a: &Element| a.attr("href"))
                        .map(|s: String| String::from(s.trim_end_matches('#')))
                        .unwrap_or_default();
                    let raw_title = anchor.and_then(|a: Element| a.text()).unwrap_or_default();
                    let raw_title = raw_title.trim();
                    let chapter_number: Option<f32> = raw_title
                        .split_whitespace()
                        .rev()
                        .find_map(|w| w.parse::<f32>().ok());
                    let title = {
                        let b = raw_title.as_bytes();
                        let is_generic = b.len() >= 3
                            && (b[0] == b'C' || b[0] == b'c')
                            && (b[1] == b'h' || b[1] == b'H');
                        if is_generic {
                            None
                        } else {
                            Some(String::from(raw_title))
                        }
                    };
                    let date_uploaded = item
                        .select_first(".chapter-release-date a")
                        .and_then(|a: Element| a.attr("title"))
                        .and_then(|s: String| relative_date_to_timestamp(s.trim()));

                    if !key.is_empty() {
                        chapters.push(Chapter {
                            key,
                            title,
                            chapter_number,
                            date_uploaded,
                            ..Default::default()
                        });
                    }
                }
            }

            updated.chapters = Some(chapters);
        }

        Ok(updated)
    }

    fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
        let url = chapter.url.unwrap_or(chapter.key);
        let html = Request::get(&url)?.html()?;
        let mut pages: Vec<Page> = Vec::new();

        if let Some(imgs) = html.select(".reading-content img") {
            for img in imgs {
                let src = img
                    .attr("data-src")
                    .or_else(|| img.attr("src"))
                    .map(|s: String| String::from(s.trim()))
                    .unwrap_or_default();

                if !src.is_empty() && (src.starts_with("http") || src.starts_with("//")) {
                    let final_src = if src.starts_with("//") {
                        format!("https:{}", src)
                    } else {
                        src
                    };
                    pages.push(Page {
                        content: PageContent::url(final_src),
                        ..Default::default()
                    });
                }
            }
        }

        Ok(pages)
    }
}

impl ImageRequestProvider for AquaReader {
    fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
        Ok(Request::get(&url)?.header("Referer", BASE_URL))
    }
}

impl ListingProvider for AquaReader {
    fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
        let url = build_listing_url(&listing.name, page);
        if listing.name == "Latest Completed" {
            parse_search_list(&url)
        } else {
            parse_manga_list(&url)
        }
    }
}

fn parse_latest_updates(page: i32) -> Result<Vec<MangaWithChapter>> {
    let url = if page <= 1 {
        format!("{}/", BASE_URL)
    } else {
        format!("{}/page/{}/", BASE_URL, page)
    };
    let html = Request::get(&url)?.html()?;
    let mut entries: Vec<MangaWithChapter> = Vec::new();

    if let Some(items) = html.select(".page-item-detail") {
        for item in items {
            let key = item
                .select_first("a")
                .and_then(|a: Element| a.attr("href"))
                .map(|s: String| String::from(s.trim_end_matches('#')))
                .unwrap_or_default();
            let title = item
                .select_first(".post-title")
                .and_then(|el: Element| el.text())
                .unwrap_or_default();
            let cover = item
                .select_first("img")
                .and_then(|img: Element| img.attr("data-src").or_else(|| img.attr("abs:src")));
            let chapter_key = item
                .select_first(".chapter-item a")
                .and_then(|a: Element| a.attr("href"))
                .map(|s: String| String::from(s.trim_end_matches('#')))
                .unwrap_or_default();
            let chapter_title = item
                .select_first(".chapter-item a")
                .and_then(|a: Element| a.text());
            let chapter_date = item
                .select_first(".post-on .c-new-tag")
                .and_then(|a: Element| a.attr("title"))
                .and_then(|s: String| relative_date_to_timestamp(s.trim()));

            if !key.is_empty() && !title.is_empty() {
                entries.push(MangaWithChapter {
                    manga: Manga {
                        key,
                        title,
                        cover,
                        ..Default::default()
                    },
                    chapter: Chapter {
                        key: chapter_key,
                        title: chapter_title,
                        date_uploaded: chapter_date,
                        ..Default::default()
                    },
                });
            }
        }
    }
    Ok(entries)
}

impl Home for AquaReader {
    fn get_home(&self) -> Result<HomeLayout> {
        let mut components: Vec<HomeComponent> = Vec::new();
        let latest_entries = parse_latest_updates(1)?;
        let latest_capped: Vec<MangaWithChapter> = latest_entries.into_iter().take(10).collect();
        components.push(HomeComponent {
            title: Some(String::from("Latest Updates")),
            value: HomeComponentValue::MangaChapterList {
                page_size: Some(5),
                entries: latest_capped,
                listing: Some(make_listing("Latest Updates", "Latest Updates")),
            },
            ..Default::default()
        });
        let popular = parse_manga_list(&format!("{}/manga/?m_orderby=views", BASE_URL))?;
        let mut popular_entries: Vec<Manga> = Vec::new();
        for manga in popular.entries.into_iter().take(5) {
            let detail_url =
                String::from(manga.key.trim_end_matches('#').trim_end_matches('/')) + "/";
            if let Ok(detail_html) = Request::get(&detail_url).and_then(|r| r.html()) {
                let description = detail_html
                    .select_first(".summary__content p")
                    .and_then(|el: Element| el.text())
                    .or_else(|| {
                        detail_html
                            .select_first(".description-summary p")
                            .and_then(|el: Element| el.text())
                    });
                let cover = detail_html
                    .select_first(".summary_image img")
                    .and_then(|img: Element| img.attr("data-src").or_else(|| img.attr("abs:src")))
                    .or(manga.cover);
                let tags: Option<Vec<String>> = detail_html
                    .select(".genres-content a")
                    .map(|els| {
                        els.map(|el: Element| String::from(el.text().unwrap_or_default().trim()))
                            .filter(|s: &String| !s.is_empty())
                            .collect()
                    })
                    .filter(|v: &Vec<String>| !v.is_empty());
                popular_entries.push(Manga {
                    key: manga.key,
                    title: manga.title,
                    cover,
                    description,
                    tags,
                    ..Default::default()
                });
            } else {
                popular_entries.push(manga);
            }
        }
        components.push(HomeComponent {
            title: Some(String::from("Popular Series")),
            value: HomeComponentValue::BigScroller {
                entries: popular_entries,
                auto_scroll_interval: Some(5.0),
            },
            ..Default::default()
        });
        let new_manga = parse_manga_list(&format!("{}/manga/?m_orderby=new-manga", BASE_URL))?;
        components.push(HomeComponent {
            title: Some(String::from("New Comic Releases")),
            value: HomeComponentValue::Scroller {
                entries: manga_to_links(new_manga.entries),
                listing: Some(make_listing("New Releases", "New Releases")),
            },
            ..Default::default()
        });
        let trending = parse_manga_list(&format!("{}/manga/?m_orderby=trending", BASE_URL))?;
        components.push(HomeComponent {
            title: Some(String::from("Trending")),
            value: HomeComponentValue::Scroller {
                entries: manga_to_links(trending.entries),
                listing: Some(make_listing("Trending", "Trending")),
            },
            ..Default::default()
        });
        let completed = parse_search_list(&format!(
            "{}/page/1/?s&post_type=wp-manga&status[]=end&m_orderby=modified",
            BASE_URL
        ))?;
        components.push(HomeComponent {
            title: Some(String::from("Latest Completed")),
            value: HomeComponentValue::Scroller {
                entries: manga_to_links(completed.entries),
                listing: Some(make_listing("Latest Completed", "Latest Completed")),
            },
            ..Default::default()
        });
        components.push(HomeComponent {
            title: Some(String::from("Browse")),
            value: HomeComponentValue::Filters(vec![
                FilterItem {
                    title: String::from("Manga"),
                    values: Some(vec![FilterValue::Sort {
                        id: String::from("type"),
                        index: 1,
                        ascending: false,
                    }]),
                },
                FilterItem {
                    title: String::from("Manhwa"),
                    values: Some(vec![FilterValue::Sort {
                        id: String::from("type"),
                        index: 2,
                        ascending: false,
                    }]),
                },
                FilterItem {
                    title: String::from("Manhua"),
                    values: Some(vec![FilterValue::Sort {
                        id: String::from("type"),
                        index: 3,
                        ascending: false,
                    }]),
                },
                FilterItem {
                    title: String::from("Completed"),
                    values: Some(vec![FilterValue::Select {
                        id: String::from("status"),
                        value: String::from("Completed"),
                    }]),
                },
                FilterItem {
                    title: String::from("Ongoing"),
                    values: Some(vec![FilterValue::Select {
                        id: String::from("status"),
                        value: String::from("Ongoing"),
                    }]),
                },
                FilterItem {
                    title: String::from("Action"),
                    values: Some(vec![FilterValue::MultiSelect {
                        id: String::from("genre"),
                        included: vec![String::from("Action")],
                        excluded: vec![],
                    }]),
                },
                FilterItem {
                    title: String::from("Adventure"),
                    values: Some(vec![FilterValue::MultiSelect {
                        id: String::from("genre"),
                        included: vec![String::from("Adventure")],
                        excluded: vec![],
                    }]),
                },
                FilterItem {
                    title: String::from("Comedy"),
                    values: Some(vec![FilterValue::MultiSelect {
                        id: String::from("genre"),
                        included: vec![String::from("Comedy")],
                        excluded: vec![],
                    }]),
                },
                FilterItem {
                    title: String::from("Drama"),
                    values: Some(vec![FilterValue::MultiSelect {
                        id: String::from("genre"),
                        included: vec![String::from("Drama")],
                        excluded: vec![],
                    }]),
                },
                FilterItem {
                    title: String::from("Fantasy"),
                    values: Some(vec![FilterValue::MultiSelect {
                        id: String::from("genre"),
                        included: vec![String::from("Fantasy")],
                        excluded: vec![],
                    }]),
                },
                FilterItem {
                    title: String::from("Romance"),
                    values: Some(vec![FilterValue::MultiSelect {
                        id: String::from("genre"),
                        included: vec![String::from("Romance")],
                        excluded: vec![],
                    }]),
                },
                FilterItem {
                    title: String::from("Isekai"),
                    values: Some(vec![FilterValue::MultiSelect {
                        id: String::from("genre"),
                        included: vec![String::from("Isekai")],
                        excluded: vec![],
                    }]),
                },
                FilterItem {
                    title: String::from("Supernatural"),
                    values: Some(vec![FilterValue::MultiSelect {
                        id: String::from("genre"),
                        included: vec![String::from("Supernatural")],
                        excluded: vec![],
                    }]),
                },
                FilterItem {
                    title: String::from("Horror"),
                    values: Some(vec![FilterValue::MultiSelect {
                        id: String::from("genre"),
                        included: vec![String::from("Horror")],
                        excluded: vec![],
                    }]),
                },
                FilterItem {
                    title: String::from("Mystery"),
                    values: Some(vec![FilterValue::MultiSelect {
                        id: String::from("genre"),
                        included: vec![String::from("Mystery")],
                        excluded: vec![],
                    }]),
                },
                FilterItem {
                    title: String::from("Psychological"),
                    values: Some(vec![FilterValue::MultiSelect {
                        id: String::from("genre"),
                        included: vec![String::from("Psychological")],
                        excluded: vec![],
                    }]),
                },
                FilterItem {
                    title: String::from("Martial Arts"),
                    values: Some(vec![FilterValue::MultiSelect {
                        id: String::from("genre"),
                        included: vec![String::from("Martial Arts")],
                        excluded: vec![],
                    }]),
                },
                FilterItem {
                    title: String::from("School Life"),
                    values: Some(vec![FilterValue::MultiSelect {
                        id: String::from("genre"),
                        included: vec![String::from("School Life")],
                        excluded: vec![],
                    }]),
                },
                FilterItem {
                    title: String::from("Sci-fi"),
                    values: Some(vec![FilterValue::MultiSelect {
                        id: String::from("genre"),
                        included: vec![String::from("Sci-fi")],
                        excluded: vec![],
                    }]),
                },
                FilterItem {
                    title: String::from("Slice-of-Life"),
                    values: Some(vec![FilterValue::MultiSelect {
                        id: String::from("genre"),
                        included: vec![String::from("Slice-of-Life")],
                        excluded: vec![],
                    }]),
                },
                FilterItem {
                    title: String::from("Thriller"),
                    values: Some(vec![FilterValue::MultiSelect {
                        id: String::from("genre"),
                        included: vec![String::from("Thriller")],
                        excluded: vec![],
                    }]),
                },
                FilterItem {
                    title: String::from("Tragedy"),
                    values: Some(vec![FilterValue::MultiSelect {
                        id: String::from("genre"),
                        included: vec![String::from("Tragedy")],
                        excluded: vec![],
                    }]),
                },
                FilterItem {
                    title: String::from("Historical"),
                    values: Some(vec![FilterValue::MultiSelect {
                        id: String::from("genre"),
                        included: vec![String::from("Historical")],
                        excluded: vec![],
                    }]),
                },
                FilterItem {
                    title: String::from("Regression"),
                    values: Some(vec![FilterValue::MultiSelect {
                        id: String::from("genre"),
                        included: vec![String::from("Regression")],
                        excluded: vec![],
                    }]),
                },
                FilterItem {
                    title: String::from("Reincarnation"),
                    values: Some(vec![FilterValue::MultiSelect {
                        id: String::from("genre"),
                        included: vec![String::from("Reincarnation")],
                        excluded: vec![],
                    }]),
                },
                FilterItem {
                    title: String::from("Survival"),
                    values: Some(vec![FilterValue::MultiSelect {
                        id: String::from("genre"),
                        included: vec![String::from("Survival")],
                        excluded: vec![],
                    }]),
                },
                FilterItem {
                    title: String::from("System"),
                    values: Some(vec![FilterValue::MultiSelect {
                        id: String::from("genre"),
                        included: vec![String::from("System")],
                        excluded: vec![],
                    }]),
                },
            ]),
            ..Default::default()
        });

        Ok(HomeLayout { components })
    }
}

impl DeepLinkHandler for AquaReader {
    fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
        let http_url = url.replacen("aidoku://", "https://", 1);
        if http_url.contains("/manga/") {
            Ok(Some(DeepLinkResult::Manga { key: http_url }))
        } else {
            Ok(None)
        }
    }
}
fn manga_to_links(entries: Vec<Manga>) -> Vec<Link> {
    entries
        .into_iter()
        .map(|m| Link {
            title: m.title,
            image_url: m.cover,
            value: Some(LinkValue::Manga(Manga {
                key: m.key,
                ..Default::default()
            })),
            ..Default::default()
        })
        .collect()
}
fn genre_to_slug(genre: &str) -> &str {
    match genre {
        "Academy" => "academy",
        "Action" => "action",
        "Adaptation" => "adaptation",
        "Adventure" => "adventure",
        "Comedy" => "comedy",
        "Cooking" => "cooking",
        "Crime" => "crime",
        "Cultivation" => "cultivation",
        "Delinquents" => "delinquents",
        "Demons" => "demons",
        "Drama" => "drama",
        "Dungeons" => "dungeons",
        "Ecchi" => "ecchi",
        "Fantasy" => "fantasy",
        "Game" => "game",
        "Gore" => "gore",
        "Harem" => "harem",
        "Historical" => "historical",
        "Horror" => "horror",
        "Isekai" => "isekai",
        "Josei" => "josei",
        "Magic" => "magic",
        "Manga" => "manga",
        "Manhua" => "manhua",
        "Manhwa" => "manhwa",
        "Martial Arts" => "martial-arts",
        "Mecha" => "mecha",
        "Medical" => "medical",
        "Military" => "military",
        "Monsters" => "monsters",
        "Murim" => "murim",
        "Music" => "music",
        "Mystery" => "mystery",
        "Necromancer" => "necromancer",
        "Ninja" => "ninja",
        "Office Workers" => "office-workers",
        "OP-MC" => "op-mc",
        "Overpowered" => "overpowered",
        "Philosophical" => "philosophical",
        "Post-Apocalyptic" => "post-apocalyptic",
        "Psychological" => "psychological",
        "Rebirth" => "rebirth",
        "Regression" => "regression",
        "Reincarnation" => "reincarnation",
        "Returner" => "returner",
        "Revenge" => "revenge",
        "Romance" => "romance",
        "School Life" => "school-life",
        "Sci-fi" => "sci-fi",
        "Seinen" => "seinen",
        "Shounen" => "shounen",
        "Slice-of-Life" => "slice-of-life",
        "Sports" => "sports",
        "Super Power" => "super-power",
        "Superhero" => "superhero",
        "Supernatural" => "supernatural",
        "Survival" => "survival",
        "System" => "system",
        "Thriller" => "thriller",
        "Time Travel" => "time-travel",
        "Tower" => "tower",
        "Tragedy" => "tragedy",
        "Vampire" => "vampire",
        "Video Games" => "video-games",
        "Villainess" => "villainess",
        "Virtual Reality" => "virtual-reality",
        "Voilence" => "violence",
        "Webcomic" => "webcomic",
        "Wuxia" => "wuxia",
        "Zombies" => "zombies",
        _ => "",
    }
}

register_source!(
    AquaReader,
    ListingProvider,
    Home,
    DeepLinkHandler,
    ImageRequestProvider
);
