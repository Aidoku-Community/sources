use crate::models::*;
use aidoku::{
    alloc::{String, Vec, vec},
    helpers::uri::QueryParameters,
    prelude::*,
    Chapter, DeepLinkResult, FilterValue, HomeComponent, HomeComponentValue, HomeLayout, Listing, ListingKind, Manga,
    MangaPageResult, Page, Result, Source, Home, ListingProvider, DeepLinkHandler, PageContent,
};

const PER_PAGE: i32 = 18;
const BASE_URL: &str = "https://manhwaweb.com";
const BACKEND_URL: &str = "https://manhwawebbackend-production.up.railway.app";

pub struct ManhwaWeb;

impl Source for ManhwaWeb {
    fn new() -> Self {
        Self
    }

    fn get_search_manga_list(
        &self,
        query: Option<String>,
        page: i32,
        filters: Vec<FilterValue>,
    ) -> Result<MangaPageResult> {
        // API is 0-based, Aidoku is 1-based.
        let api_page = if page > 0 { page - 1 } else { 0 };
        let mut url = format!("{BACKEND_URL}/manhwa/library?page={}&perPage={}", api_page, PER_PAGE);
        let mut qs = QueryParameters::new();
        
        // Handle search query
        if let Some(q) = query {
             let trimmed = q.trim();
             if !trimmed.is_empty() {
                 qs.push("buscar", Some(trimmed));
             }
        }

        let mut erotic_filter_set = false;
        let mut genre_values: Vec<String> = Vec::new();

        for filter in filters {
            match filter {
                FilterValue::Select { id, value } => {
                    if !value.is_empty() {
                         // Pass ID directly as API expects (e.g., 'tipo', 'demografia')
                         qs.push(&id, Some(&value));
                         
                         if id == "erotico" {
                             erotic_filter_set = true;
                         }
                    }
                }
                FilterValue::MultiSelect { id, included, .. } => {
                    // For genres
                    if id == "genres" || id == "genreIds" || id == "generes" {
                        for val in included {
                            genre_values.push(val);
                        }
                    }
                }
                 FilterValue::Sort { index, .. } => {
                     let sort_val = match index {
                         0 => "alfabetico",
                         2 => "num_chapter",
                         _ => "creacion", // Default to creation date
                     };
                     qs.push("order_item", Some(sort_val));
                }
                _ => {}
            }
        }
        
        // Handle Genres: joined by 'a' (e.g., "1a2a3")
        if !genre_values.is_empty() {
            let joined_genres = genre_values.join("a");
            qs.push("generes", Some(&joined_genres));
        }

        // Default to "no" erotic content if the filter wasn't explicitly set
        if !erotic_filter_set {
            qs.push("erotico", Some("no"));
        }
        
        // Ensure qs is appended with '&' prefix because base URL might have query params already.
        let qs_str = format!("{}", qs);
        if !qs_str.is_empty() {
            url.push_str(&format!("&{}", qs_str));
        }

        let mut response = crate::helper::request_with_limits(&url, "GET")?
            .header("Referer", &format!("{}/", BASE_URL))
            .send()?;

        let data = response.get_json::<LibraryResponse>()?;
        let entries = data.data.iter().map(|m| m.to_manga(BASE_URL)).collect();
        // Pagination check: API returns 'next': boolean.
        let has_next_page = data.next;

        Ok(MangaPageResult {
            entries,
            has_next_page,
        })
    }

    fn get_manga_update(
        &self,
        mut manga: Manga,
        needs_details: bool,
        needs_chapters: bool,
    ) -> Result<Manga> {
        let url = format!("{BACKEND_URL}/manhwa/see/{}", manga.key);
        let mut response = crate::helper::request_with_limits(&url, "GET")?
            .header("Referer", &format!("{}/", BASE_URL))
            .send()?;
        let data = response.get_json::<SeeResponse>()?;

        if needs_details {
            manga.copy_from(data.parse_manga(BASE_URL));
        }

        if needs_chapters {
            manga.chapters = Some(data.parse_chapters(BASE_URL));
        }

        Ok(manga)
    }

    fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
         let url = format!("{BACKEND_URL}/chapters/see/{}", chapter.key);
        let mut response = crate::helper::request_with_limits(&url, "GET")?
            .send()?;
        let data = response.get_json::<ChapterSeeResponse>()?;
        
        Ok(data.chapter.img.into_iter().enumerate().map(|(_i, url)| Page {
            content: PageContent::Url(url, None),
            ..Default::default()
        }).collect())
    }
}

impl DeepLinkHandler for ManhwaWeb {
    fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
         if let Some(path) = url.strip_prefix(BASE_URL) {
            if path.starts_with("/manhwa/") {
                let id = path.trim_start_matches("/manhwa/");
                return Ok(Some(DeepLinkResult::Manga {
                    key: id.into(),
                }));
            }
         }
         Ok(None)
    }
}

use aidoku::alloc::string::ToString;

/// Helper function to map genre IDs to Spanish names.
fn get_genre_name(id: &str) -> String {
    match id {
        "3" => "Acción", "29" => "Aventura", "18" => "Comedia", "1" => "Drama",
        "42" => "Recuentos de la vida", "2" => "Romance", "5" => "Venganza", "6" => "Harem",
        "23" => "Fantasía", "31" => "Sobrenatural", "25" => "Tragedia", "43" => "Psicológico",
        "32" => "Horror", "44" => "Thriller", "28" => "Historias cortas", "30" => "Ecchi",
        "34" => "Gore", "37" => "Girls love", "27" => "Boys love", "45" => "Reencarnación",
        "41" => "Sistema de niveles", "33" => "Ciencia ficción", "38" => "Apocalíptico",
        "39" => "Artes marciales", "40" => "Superpoderes", "35" => "Cultivación (cultivo)",
        "8" => "Milf", _ => "Desconocido",
    }.to_string()
}

impl Home for ManhwaWeb {
    fn get_home(&self) -> Result<HomeLayout> {
        let mut components = Vec::new();

        // 1. Hero: Latest Chapters ("Nuevos Capitulos") - BigScroller
        let url_latest = format!("{BACKEND_URL}/manhwa/nuevos");
        if let Ok(response) = crate::helper::request_with_limits(&url_latest, "GET") {
             if let Ok(mut resp) = response.send() {
                if let Ok(data) = resp.get_json::<NuevosResponse>() {
                    let latest_entries: Vec<Manga> = data.manhwas.spanish_manhwas.iter().map(|m| {
                        let group = m.gru_name.clone().unwrap_or_default();
                        let subtitle = format!("Cap. {} • {}", m.chapter, group);

                         Manga {
                            key: m.id_manhwa.clone().into(),
                            title: m.name_manhwa.clone(),
                            authors: Some(vec![subtitle]), 
                            cover: m.img.clone().map(|s| s.into()),
                            url: Some(format!("{}/manhwa/{}", BASE_URL, m.id_manhwa)),
                            ..Default::default()
                        }
                    }).collect();

                    components.push(HomeComponent {
                        title: Some("Nuevos Capítulos".into()),
                        subtitle: None,
                        value: HomeComponentValue::BigScroller {
                            entries: latest_entries,
                            auto_scroll_interval: Some(5.0),
                        },
                    });
                }
             }
        }

        // 2. New Works ("Nuevas Obras") - Scroller
        let url_new = format!("{BACKEND_URL}/manhwa/library?page=0&perPage=12&order_item=creacion&order_dir=desc");
        if let Ok(resp) = crate::helper::request_with_limits(&url_new, "GET") {
            if let Ok(mut resp) = resp.send() {
                if let Ok(data) = resp.get_json::<LibraryResponse>() {
                    let entries: Vec<aidoku::Link> = data.data
                        .iter()
                        .filter(|m| m.erotic.as_deref() != Some("si"))
                        .map(|m| {
                             let mut manga = m.to_manga(BASE_URL);
                             if let Some(cats) = &m.categories {
                                 let tags: Vec<String> = cats.iter().map(|id| get_genre_name(&id.to_string())).collect();
                                 manga.tags = Some(tags);
                             }
                             manga.into()
                        })
                        .collect();

                    components.push(HomeComponent {
                        title: Some("Nuevas Obras".into()),
                        subtitle: None,
                        value: HomeComponentValue::Scroller {
                             entries,
                             listing: Some(Listing {
                                 id: "New".into(),
                                 name: "Nuevas Obras".into(),
                                 kind: ListingKind::Default,
                             }), 
                        },
                    });
                }
            }
        }

        Ok(HomeLayout { components })
    }
}

impl ListingProvider for ManhwaWeb {
    fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
        let name = listing.id.as_str(); 
        let api_page = if page > 0 { page - 1 } else { 0 };

        if name.starts_with("Genre:") {
             let id = name.split(':').nth(1).unwrap_or("");
             // Ensure 'creacion' (creation date) sort is properly applied for these listings too
             let url = format!("{BACKEND_URL}/manhwa/library?page={}&perPage={}&erotico=no&generes={}&order_item=creacion&order_dir=desc", api_page, PER_PAGE, id);
             let resp = crate::helper::request_with_limits(&url, "GET")?.send()?.get_json::<LibraryResponse>()?;
             let entries: Vec<Manga> = resp.data.iter().map(|m| m.to_manga(BASE_URL)).collect();
             return Ok(MangaPageResult { entries, has_next_page: resp.next });
        }

        match name {
            // "Latest" and "Popular" standard tabs fallback
            // Note: The API does not strictly support a "Popular" endpoint that differs significantly from "Nuevos" in this context without specific implementation.
            "Latest" | "Popular" => {
                 if page > 1 {
                     return Ok(MangaPageResult { entries: vec![], has_next_page: false });
                 }
                 let url = format!("{BACKEND_URL}/manhwa/nuevos");
                 let resp = crate::helper::request_with_limits(&url, "GET")?.send()?.get_json::<NuevosResponse>()?;
                 let entries: Vec<Manga> = resp.manhwas.spanish_manhwas.iter().map(|m| Manga {
                            key: m.id_manhwa.clone().into(),
                            title: m.name_manhwa.clone(),
                            cover: m.img.clone().map(|s| s.into()),
                            url: Some(format!("{}/manhwa/{}", BASE_URL, m.id_manhwa)),
                            ..Default::default()
                 }).collect();
                 Ok(MangaPageResult { entries, has_next_page: false })
            }
             "New" => {
                 self.get_search_manga_list(None, page, vec![FilterValue::Sort { index: 3, ascending: false, id: "sortBy".into() }])
             }
             "Erotic" | "+18 (Erotic)" => {
                 let url = format!("{BACKEND_URL}/manhwa/library?page={}&perPage={}&erotico=si", api_page, PER_PAGE);
                 let data = crate::helper::request_with_limits(&url, "GET")?.header("Referer", &format!("{}/", BASE_URL)).send()?.get_json::<LibraryResponse>()?;
                 let entries = data.data.iter().map(|m| m.to_manga(BASE_URL)).collect();
                 Ok(MangaPageResult { entries, has_next_page: data.next })
             }
             // Search fallback
            _ => {
                 let filters = vec![];
                 self.get_search_manga_list(None, page, filters)
            }
        }
    }
}
