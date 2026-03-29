#![no_std]
use aidoku::{
    Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterItem, FilterValue, Home,
    HomeComponent, HomeComponentValue, HomeLayout, ImageRequestProvider, Link, LinkValue, Listing,
    ListingKind, ListingProvider, Manga, MangaPageResult, MangaStatus, MangaWithChapter, Page,
    PageContent, PageContext, Result, Source, Viewer,
    alloc::{String, Vec, format, vec},
    imports::{html::Element, net::Request},
    prelude::*,
};

const BASE_URL: &str = "https://bakamh.com";

struct Bakamh;

fn make_listing(id: &str, name: &str) -> Listing {
    Listing {
        id: String::from(id),
        name: String::from(name),
        kind: ListingKind::Default,
    }
}

fn build_listing_url(category: &str, page: i32) -> String {
    let slug = match category {
        "BL" => "bl",
        "GL" => "gl",
        "All Ages" => "allages",
        "English" => "en-manga",
        "New Manga" => "newmanga",
        "Latest Updates" => "",
        "Completed" | "Completed Manhwa" => "end",
        _ => "",
    };
    if slug.is_empty() {
        if page <= 1 {
            format!("{}/", BASE_URL)
        } else {
            format!("{}/page/{}/", BASE_URL, page)
        }
    } else if page <= 1 {
        format!("{}/{}/", BASE_URL, slug)
    } else {
        format!("{}/{}/page/{}/", BASE_URL, slug, page)
    }
}

fn parse_manga_list(url: &str) -> Result<MangaPageResult> {
    let html = Request::get(url)?.html()?;
    let mut entries: Vec<Manga> = Vec::new();

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
                .and_then(|img: Element| img.attr("abs:src"));

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

    let has_next_page = html.select_first("a[class*=\"next\"]").is_some();

    Ok(MangaPageResult {
        entries,
        has_next_page,
    })
}

fn get_status(html: &aidoku::imports::html::Document) -> MangaStatus {
    if let Some(items) = html.select(".post-content_item") {
        for item in items {
            let heading = item
                .select_first(".summary-heading h5")
                .and_then(|h: Element| h.text())
                .unwrap_or_default();
            if (heading.trim() == "Status" || heading.trim() == "状态")
                && let Some(status_text) = item
                    .select_first(".summary-content")
                    .and_then(|el: Element| el.text())
            {
                return match status_text.trim() {
                    "连载中" | "Serialization" | "OnGoing" | "ongoing" => MangaStatus::Ongoing,
                    "已完结" | "Completed" | "completed" | "完结" => MangaStatus::Completed,
                    "停载" | "Cancelled" | "cancelled" => MangaStatus::Cancelled,
                    "休刊" | "Hiatus" | "hiatus" => MangaStatus::Hiatus,
                    _ => MangaStatus::Unknown,
                };
            }
        }
    }
    MangaStatus::Unknown
}

impl Source for Bakamh {
    fn new() -> Self {
        Self
    }

    fn get_search_manga_list(
        &self,
        _query: Option<String>,
        _page: i32,
        _filters: Vec<FilterValue>,
    ) -> Result<MangaPageResult> {
        let mut sort = String::from("latest");
        let mut status = String::new();
        let mut genre = String::new();
        let mut tags_params = String::new();
        let mut gl_redirect = false;
        let mut listing_redirect: Option<String> = None;

        for filter in _filters {
            match filter {
                FilterValue::Sort { index, .. } => {
                    sort = match index {
                        0 => String::from("new-manga"),
                        1 => String::from("latest"),
                        2 => String::from("views"),
                        _ => String::from("latest"),
                    };
                }
                FilterValue::Select { id, value } if id == "status" => {
                    status = match value.as_str() {
                        "Completed" => String::from("&status[]=end"),
                        "Ongoing" => String::from("&status[]=on-going"),
                        _ => String::new(),
                    };
                }
                FilterValue::Select { id, value } if id == "genre" => match value.as_str() {
                    "New Manga" | "BL" | "All Ages" | "English" | "Completed Manhwa"
                    | "Latest Updates" => {
                        listing_redirect = Some(value);
                    }
                    "GL" => {
                        gl_redirect = true;
                    }
                    _ => {
                        genre = match value.as_str() {
                            "BL" => String::from("&genre[]=bl"),
                            "All Ages" => String::from("&genre[]=%e5%85%a8%e5%b9%b4%e9%be%84"),
                            "English" => String::from("&genre[]=en-manga"),
                            "Animation" => String::from("&genre[]=%e5%8a%a8%e7%94%bb"),
                            "Doujinshi (Chinese)" => String::from(
                                "&genre[]=%e6%b1%89%e5%8c%96%e5%90%8c%e4%ba%ba%e5%bf%97",
                            ),
                            "Translated JP Manga" => {
                                String::from("&genre[]=%e6%b1%89%e5%8c%96%e6%97%a5%e6%bc%ab")
                            }
                            "Korean Comics" => String::from("&genre[]=%e9%9f%a9%e6%bc%ab"),
                            _ => String::new(),
                        };
                    }
                },
                FilterValue::MultiSelect { id, included, .. } if id == "tags" => {
                    for tag in included {
                        let slug = match tag.as_str() {
                            "Drama" => "drama",
                            "Romance" => "romance",
                            "Comedy" => "comedy",
                            "Affair" => "affair",
                            "College" => "college",
                            "Friends" => "friends",
                            "Love" => "love",
                            "Secret" => "secret",
                            "Triangle" => "triangle",
                            "Age Gap" => "age",
                            "Teacher" => "teacher",
                            "Fetish" => "fetish",
                            "Furry" => "furry",
                            "Hardcore" => "hardcore",
                            "Tentacles" => "tentacles",
                            "Married" => "married",
                            "GL" => "gl",
                            "Ancient Style" => "%e5%8f%a4%e9%a3%8e",
                            "Cool Girl" => "%e5%b8%85%e6%b0%94%e5%a5%b3",
                            "Gentle Heroine" => "%e6%b8%a9%e6%9f%94%e5%a5%b3%e4%b8%bb",
                            "Pure Heroine" => "%e7%ba%af%e6%83%85%e5%a5%b3%e4%b8%bb",
                            "Rich Heroine" => "%e8%b1%aa%e9%97%a8%e5%a5%b3%e4%b8%bb",
                            "Trauma" => "%e5%88%9b%e4%bc%a4%e5%8f%97",
                            "Living Together" => "%e5%90%8c%e5%b1%85",
                            "Older Uke" => "%e5%b9%b4%e4%b8%8a%e5%8f%97",
                            "Younger Seme" => "%e5%b9%b4%e4%b8%8b%e6%94%bb",
                            "Age Difference" => "%e5%b9%b4%e9%be%84%e5%b7%ae",
                            "Daily Life" => "%e6%97%a5%e5%b8%b8",
                            "Gentle Seme" => "%e6%b8%a9%e6%9f%94%e6%94%bb",
                            "Modern" => "%e7%8e%b0%e4%bb%a3",
                            "Pure Seme" => "%e7%ba%af%e6%83%85%e6%94%bb",
                            "Neighbor" => "%e9%82%bb%e5%b1%85",
                            "Deadpan Uke" => "%e9%9d%a2%e7%98%ab%e5%8f%97",
                            _ => "",
                        };
                        if !slug.is_empty() {
                            tags_params.push_str(&format!("&genre[]={}", slug));
                        }
                    }
                }
                _ => {}
            }
        }

        if let Some(listing_name) = listing_redirect {
            return parse_manga_list(&build_listing_url(&listing_name, _page));
        }

        if gl_redirect {
            return parse_manga_list(&format!("{}/gl/page/{}/", BASE_URL, _page));
        }

        let query = _query.unwrap_or_default();
        let url = format!(
            "{}/page/{}/?s={}&post_type=wp-manga&m_orderby={}&op={}{}{}{}",
            BASE_URL, _page, query, sort, "", status, genre, tags_params
        );

        let html = Request::get(&url)?.html()?;
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
                    .and_then(|img: Element| img.attr("abs:src"));

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

        let has_next_page = html.select_first("a[class*=\"next\"]").is_some();
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
        let url = String::from(manga.key.trim_end_matches('#').trim_end_matches('/')) + "/";
        let html = Request::get(&url)?.html()?;
        let mut updated = manga;

        if needs_details {
            if let Some(title) = html
                .select_first("#manga-title h1")
                .and_then(|el: Element| el.text())
            {
                updated.title = title;
            }

            updated.cover = html
                .select_first(".summary_image img")
                .and_then(|img: Element| img.attr("abs:src"))
                .or(updated.cover);

            updated.description = html
                .select_first(".post-content_item p")
                .and_then(|el: Element| el.text());

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
            updated.content_rating = ContentRating::NSFW;

            let mut all_tags: Vec<String> = Vec::new();

            if let Some(items) = html.select(".post-content_item") {
                for item in items {
                    let heading = item
                        .select_first(".summary-heading h5")
                        .and_then(|h: Element| h.text())
                        .unwrap_or_default();
                    let heading = heading.trim();

                    if (heading == "分类" || heading == "Classification")
                        && let Some(els) = item.select(".genres-content a")
                    {
                        for el in els {
                            let text = String::from(el.text().unwrap_or_default().trim());
                            if !text.is_empty() {
                                all_tags.push(text);
                            }
                        }
                    }

                    if (heading == "标籤"
                        || heading == "Mark"
                        || heading == "Tags"
                        || heading == "标签"
                        || heading == "标记")
                        && let Some(els) = item.select(".tags-content a")
                    {
                        for el in els {
                            let text = String::from(el.text().unwrap_or_default().trim());
                            if !text.is_empty() {
                                all_tags.push(text);
                            }
                        }
                    }
                }
            }

            updated.tags = if all_tags.is_empty() {
                None
            } else {
                Some(all_tags)
            };
        }

        if needs_chapters {
            let mut chapters: Vec<Chapter> = Vec::new();

            if let Some(items) = html.select("li.chapter-cubical") {
                let all: Vec<_> = items.collect();
                let total = all.len();
                for (i, item) in all.into_iter().enumerate() {
                    let key = item
                        .select_first("a")
                        .and_then(|a: Element| a.attr("chapter-data-url"))
                        .unwrap_or_default();
                    let title = item.select_first("a").and_then(|a: Element| a.text());

                    if !key.is_empty() {
                        chapters.push(Chapter {
                            key: key.clone(),
                            url: Some(key),
                            title,
                            chapter_number: Some((total - i) as f32),
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
                    .attr("src")
                    .map(|s: String| String::from(s.trim()))
                    .unwrap_or_default();
                if !src.is_empty() && src.starts_with("http") {
                    pages.push(Page {
                        content: PageContent::url(src),
                        ..Default::default()
                    });
                }
            }
        }

        Ok(pages)
    }
}

impl ImageRequestProvider for Bakamh {
    fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
        Ok(Request::get(&url)?.header("Referer", BASE_URL))
    }
}

impl ListingProvider for Bakamh {
    fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
        let url = build_listing_url(&listing.name, page);
        parse_manga_list(&url)
    }
}

impl Home for Bakamh {
    fn get_home(&self) -> Result<HomeLayout> {
        let mut components: Vec<HomeComponent> = Vec::new();

        // New Manga
        let new_manga_list = parse_manga_list(&format!("{}/newmanga/", BASE_URL))?;
        let mut new_manga_entries: Vec<Manga> = Vec::new();
        for manga in new_manga_list.entries.into_iter().take(5) {
            let detail_url =
                String::from(manga.key.trim_end_matches('#').trim_end_matches('/')) + "/";
            if let Ok(detail_html) = Request::get(&detail_url).and_then(|r| r.html()) {
                let description = detail_html
                    .select_first(".post-content_item p")
                    .and_then(|el: Element| el.text());
                let cover = detail_html
                    .select_first(".summary_image img")
                    .and_then(|img: Element| img.attr("abs:src"))
                    .or(manga.cover);
                new_manga_entries.push(Manga {
                    key: manga.key,
                    title: manga.title,
                    cover,
                    description,
                    ..Default::default()
                });
            } else {
                new_manga_entries.push(manga);
            }
        }
        components.push(HomeComponent {
            title: Some(String::from("New Manga")),
            value: HomeComponentValue::BigScroller {
                entries: new_manga_entries,
                auto_scroll_interval: Some(3.0),
            },
            ..Default::default()
        });

        // Latest Updates — MangaChapterList limited to 5
        let latest_html = Request::get(format!("{}/", BASE_URL))?.html()?;
        let mut latest_entries: Vec<MangaWithChapter> = Vec::new();
        if let Some(items) = latest_html.select(".page-item-detail") {
            for item in items.take(5) {
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
                    .and_then(|img: Element| img.attr("abs:src"));
                let chapter_key = item
                    .select_first(".chapter-item a")
                    .and_then(|a: Element| a.attr("href"))
                    .unwrap_or_default();
                let chapter_title = item
                    .select_first(".chapter-item a")
                    .and_then(|a: Element| a.text());

                if !key.is_empty() && !title.is_empty() {
                    latest_entries.push(MangaWithChapter {
                        manga: Manga {
                            key,
                            title,
                            cover,
                            ..Default::default()
                        },
                        chapter: Chapter {
                            key: chapter_key.clone(),
                            url: Some(chapter_key),
                            title: chapter_title,
                            ..Default::default()
                        },
                    });
                }
            }
        }
        components.push(HomeComponent {
            title: Some(String::from("Latest Updates")),
            value: HomeComponentValue::MangaChapterList {
                page_size: None,
                entries: latest_entries,
                listing: Some(make_listing("Latest Updates", "Latest Updates")),
            },
            ..Default::default()
        });

        // BL
        let bl = parse_manga_list(&format!("{}/bl/", BASE_URL))?;
        let bl_links: Vec<Link> = bl
            .entries
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
            .collect();
        components.push(HomeComponent {
            title: Some(String::from("BL")),
            value: HomeComponentValue::Scroller {
                entries: bl_links,
                listing: Some(make_listing("BL", "BL")),
            },
            ..Default::default()
        });

        // GL (Yuri :3)
        let gl = parse_manga_list(&format!("{}/gl/", BASE_URL))?;
        let gl_links: Vec<Link> = gl
            .entries
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
            .collect();
        components.push(HomeComponent {
            title: Some(String::from("GL")),
            value: HomeComponentValue::Scroller {
                entries: gl_links,
                listing: Some(make_listing("GL", "GL")),
            },
            ..Default::default()
        });

        // All Ages
        let allages = parse_manga_list(&format!("{}/allages/", BASE_URL))?;
        let allages_links: Vec<Link> = allages
            .entries
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
            .collect();
        components.push(HomeComponent {
            title: Some(String::from("All Ages")),
            value: HomeComponentValue::Scroller {
                entries: allages_links,
                listing: Some(make_listing("All Ages", "All Ages")),
            },
            ..Default::default()
        });

        // English
        let english = parse_manga_list(&format!("{}/en-manga/", BASE_URL))?;
        let english_links: Vec<Link> = english
            .entries
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
            .collect();
        components.push(HomeComponent {
            title: Some(String::from("English")),
            value: HomeComponentValue::Scroller {
                entries: english_links,
                listing: Some(make_listing("English", "English")),
            },
            ..Default::default()
        });

        // Browse — Filters section at bottom
        components.push(HomeComponent {
            title: Some(String::from("Browse")),
            value: HomeComponentValue::Filters(vec![
                FilterItem {
                    title: String::from("Latest Updates"),
                    values: Some(vec![FilterValue::Select {
                        id: String::from("genre"),
                        value: String::from("Latest Updates"),
                    }]),
                },
                FilterItem {
                    title: String::from("New Manga"),
                    values: Some(vec![FilterValue::Select {
                        id: String::from("genre"),
                        value: String::from("New Manga"),
                    }]),
                },
                FilterItem {
                    title: String::from("BL"),
                    values: Some(vec![FilterValue::Select {
                        id: String::from("genre"),
                        value: String::from("BL"),
                    }]),
                },
                FilterItem {
                    title: String::from("GL"),
                    values: Some(vec![FilterValue::Select {
                        id: String::from("genre"),
                        value: String::from("GL"),
                    }]),
                },
                FilterItem {
                    title: String::from("All Ages"),
                    values: Some(vec![FilterValue::Select {
                        id: String::from("genre"),
                        value: String::from("All Ages"),
                    }]),
                },
                FilterItem {
                    title: String::from("English"),
                    values: Some(vec![FilterValue::Select {
                        id: String::from("genre"),
                        value: String::from("English"),
                    }]),
                },
                FilterItem {
                    title: String::from("Completed Manhwa"),
                    values: Some(vec![FilterValue::Select {
                        id: String::from("genre"),
                        value: String::from("Completed Manhwa"),
                    }]),
                },
            ]),
            ..Default::default()
        });

        Ok(HomeLayout { components })
    }
}

impl DeepLinkHandler for Bakamh {
    fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
        let http_url = url.replacen("aidoku://", "https://", 1);
        if http_url.contains("/manga/") {
            Ok(Some(DeepLinkResult::Manga { key: http_url }))
        } else {
            Ok(None)
        }
    }
}

register_source!(
    Bakamh,
    ListingProvider,
    Home,
    DeepLinkHandler,
    ImageRequestProvider
);
