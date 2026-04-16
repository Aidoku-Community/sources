#![no_std]
use aidoku::{
    AidokuError, Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeLayout, Listing,
    ListingProvider, Manga, MangaPageResult, Page, Result, Source,
    alloc::{String, Vec, format, vec},
    imports::net::Request,
    prelude::*,
};
use chrono::{DateTime, Utc};

const API_BASE: &str = "https://fmteam.fr/api";
const SITE_BASE: &str = "https://fmteam.fr";

struct Fmteamfr;

#[derive(serde::Deserialize)]
struct ListComicResponse {
    comics: Vec<ListComic>,
}

#[derive(serde::Deserialize)]
struct ListComic {
    title: String,
    thumbnail: String,
    author: Option<String>,
    artist: Option<String>,
    url: String,
    slug: String,
    updated_at: String,
    adult: i32,
    status: String,
    description: Option<String>,
    last_chapter: Option<ComicChapter>,
}

#[derive(serde::Deserialize)]
struct ComicDetailResponse {
    comic: ComicDetail,
}

#[derive(serde::Deserialize)]
struct ComicDetail {
    title: String,
    thumbnail: String,
    description: Option<String>,
    author: Option<String>,
    artist: Option<String>,
    adult: i32,
    status: String,
    url: String,
    chapters: Vec<ComicChapter>,
}

#[derive(serde::Deserialize)]
struct ComicChapter {
    full_title: Option<String>,
    title: Option<String>,
    chapter: Option<f64>,
    updated_at: Option<String>,
    published_on: Option<String>,
    url: String,
}

#[derive(serde::Deserialize)]
struct ReadResponse {
    chapter: ReadChapter,
}

#[derive(serde::Deserialize)]
struct ReadChapter {
    pages: Vec<String>,
}

fn to_manga_status(status_str: &str) -> aidoku::MangaStatus {
    match status_str {
        "En cours" => aidoku::MangaStatus::Ongoing,
        "Terminé" => aidoku::MangaStatus::Completed,
        _ => aidoku::MangaStatus::Unknown,
    }
}

fn to_manga_content_rating(adult: i32) -> aidoku::ContentRating {
    match adult {
        0 => aidoku::ContentRating::Safe,
        1 => aidoku::ContentRating::NSFW,
        _ => aidoku::ContentRating::Unknown,
    }
}

fn get_all_mangas(sorted_by_updated: bool) -> Result<Vec<Manga>> {
    let response: ListComicResponse = Request::get(format!("{API_BASE}/comics"))?.json_owned()?;

    let mut comics = response.comics;
    if sorted_by_updated {
        comics.sort_by(|a, b| {
            let a_date = a
                .last_chapter
                .as_ref()
                .and_then(|c| c.published_on.as_deref())
                .or(Some(a.updated_at.as_str()))
                .unwrap_or("");

            let b_date = b
                .last_chapter
                .as_ref()
                .and_then(|c| c.published_on.as_deref())
                .or(Some(b.updated_at.as_str()))
                .unwrap_or("");

            b_date.cmp(a_date)
        });
    }

    let mangas = comics
        .into_iter()
        .map(|comic| Manga {
            key: comic.slug,
            title: comic.title,
            description: comic.description,
            url: Some(format!("{SITE_BASE}{}", comic.url)),
            cover: Some(comic.thumbnail),
            authors: comic.author.filter(|a| !a.is_empty()).map(|a| vec![a]),
            artists: comic.artist.filter(|a| !a.is_empty()).map(|a| vec![a]),
            viewer: aidoku::Viewer::RightToLeft,
            content_rating: to_manga_content_rating(comic.adult),
            status: to_manga_status(&comic.status),
            ..Default::default()
        })
        .collect::<Vec<_>>();

    Ok(mangas)
}

impl Source for Fmteamfr {
    fn new() -> Self {
        Self
    }

    fn get_search_manga_list(
        &self,
        query: Option<String>,
        _page: i32,
        _filters: Vec<FilterValue>,
    ) -> Result<MangaPageResult> {
        let mangas = get_all_mangas(false)?;

        let query_normalized = query
            .as_deref()
            .map(str::trim)
            .map(str::to_lowercase)
            .filter(|q| !q.is_empty());

        let entries = if let Some(q) = query_normalized {
            mangas
                .into_iter()
                .filter(|manga| manga.title.to_lowercase().contains(&q))
                .collect()
        } else {
            mangas
        };

        Ok(MangaPageResult {
            entries,
            has_next_page: false,
        })
    }

    fn get_manga_update(
        &self,
        mut manga: Manga,
        needs_details: bool,
        needs_chapters: bool,
    ) -> Result<Manga> {
        let slug = manga.key.trim();
        if slug.is_empty() {
            return Err(AidokuError::RequestError(
                aidoku::imports::net::RequestError::MissingUrl,
            ));
        }

        let response: ComicDetailResponse =
            Request::get(format!("{API_BASE}/comics/{slug}"))?.json_owned()?;
        let comic = response.comic;

        if needs_details {
            manga.title = comic.title;
            manga.cover = Some(comic.thumbnail);
            manga.url = Some(format!("{SITE_BASE}{}", comic.url));
            manga.description = comic.description;
            manga.authors = comic.author.filter(|a| !a.is_empty()).map(|a| vec![a]);
            manga.artists = comic.artist.filter(|a| !a.is_empty()).map(|a| vec![a]);
            manga.status = to_manga_status(&comic.status);
            manga.content_rating = to_manga_content_rating(comic.adult);
            manga.viewer = aidoku::Viewer::RightToLeft;
        }

        if needs_chapters {
            let mut chapters: Vec<Chapter> = Vec::new();

            for chapter in comic.chapters {
                let chapter_title = chapter
                    .title
                    .filter(|t| !t.is_empty())
                    .or(chapter.full_title.filter(|t| !t.is_empty()));

                let date_uploaded = chapter
                    .published_on
                    .or(chapter.updated_at)
                    .and_then(|date| {
                        DateTime::parse_from_rfc3339(&date)
                            .ok()
                            .map(|dt| dt.with_timezone(&Utc).timestamp())
                    });

                let key = chapter.url;

                chapters.push(Chapter {
                    key,
                    title: chapter_title,
                    chapter_number: chapter.chapter.map(|n| n as f32),
                    date_uploaded,
                    ..Default::default()
                });
            }

            manga.chapters = Some(chapters);
        }

        Ok(manga)
    }

    fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
        let chapter_path = chapter.key.trim();
        if chapter_path.is_empty() {
            return Err(AidokuError::RequestError(
                aidoku::imports::net::RequestError::MissingUrl,
            ));
        }

        let response: ReadResponse =
            Request::get(format!("{API_BASE}{}", chapter_path))?.json_owned()?;

        let pages = response
            .chapter
            .pages
            .into_iter()
            .map(|url| Page {
                content: aidoku::PageContent::Url(url, None),
                ..Default::default()
            })
            .collect::<Vec<_>>();

        Ok(pages)
    }
}

impl ListingProvider for Fmteamfr {
    fn get_manga_list(&self, _listing: Listing, page: i32) -> Result<MangaPageResult> {
        self.get_search_manga_list(None, page, Vec::new())
    }
}

impl Home for Fmteamfr {
    fn get_home(&self) -> Result<HomeLayout> {
        let entries = get_all_mangas(true)?
            .into_iter()
            .take(10)
            .map(|m| m.into())
            .collect();

        Ok(HomeLayout {
            components: vec![aidoku::HomeComponent {
                title: Some("Dernières sorties".into()),
                subtitle: Some("10 dernier chapitres mis à jour".into()),
                value: aidoku::HomeComponentValue::MangaList {
                    ranking: false,
                    page_size: Some(10),
                    entries,
                    listing: None,
                },
            }],
        })
    }
}

impl DeepLinkHandler for Fmteamfr {
    fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
        if !url.starts_with(SITE_BASE) {
            return Ok(None);
        }

        if let Some(path) = url.strip_prefix(SITE_BASE) {
            let path_parts: Vec<&str> = path.trim_matches('/').split('/').collect();
            if path_parts.len() >= 2 && path_parts[0] == "comic" {
                let manga_key = path_parts[1];
                return Ok(Some(DeepLinkResult::Manga {
                    key: manga_key.into(),
                }));
            }

            if path_parts.len() >= 4 && path_parts[0] == "comic" && path_parts[2] == "chapter" {
                let manga_key = path_parts[1];
                let chapter_key = format!("/comic/{}/chapter/{}", manga_key, path_parts[3]);
                return Ok(Some(DeepLinkResult::Chapter {
                    manga_key: manga_key.into(),
                    key: chapter_key.into(),
                }));
            }
        }

        Ok(None)
    }
}

register_source!(Fmteamfr, ListingProvider, Home, DeepLinkHandler);
