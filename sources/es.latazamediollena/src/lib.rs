#![no_std]
mod helper;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent,
	HomeComponentValue, HomeLayout, Manga, MangaPageResult,
	MangaWithChapter, Page, PageContent, Result, Source,
	alloc::{String, Vec, vec},
	imports::net::Request,
	prelude::*,
};
use helper::{
	BASE_URL, MANGA_KEY, comic_info, fetch_all_chapters, fetch_archive_page,
	parse_page_description, parse_page_image_url, slug_from_url,
};

struct Latazamediollena;

impl Source for Latazamediollena {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		_page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let manga = comic_info();
		let matches = match query {
			Some(query) => {
				let query = query.to_ascii_lowercase();
				manga.title.to_ascii_lowercase().contains(&query)
			}
			_ => true,
		};

		Ok(MangaPageResult {
			entries: if matches { vec![manga] } else { Vec::new() },
			has_next_page: false,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			manga.copy_from(comic_info());
		}
		if needs_chapters {
			manga.chapters = Some(fetch_all_chapters()?);
		}
		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = chapter
			.url
			.unwrap_or_else(|| format!("{BASE_URL}/comic/{}/", chapter.key));
		let html = Request::get(url)?.html()?;
		let image_url = parse_page_image_url(&html).ok_or_else(|| {
			error!("No se ha encontrado la imagen de la tira en la página")
		})?;
		let description = parse_page_description(&html);

		Ok(vec![Page {
			content: PageContent::url(image_url),
			has_description: description.is_some(),
			description,
			..Default::default()
		}])
	}
}

impl Home for Latazamediollena {
	fn get_home(&self) -> Result<HomeLayout> {
		let manga = comic_info();
		let (chapters, _) = fetch_archive_page(&format!("{BASE_URL}/comic/"))?;

		let manga_chapters = chapters
			.into_iter()
			.map(|chapter| MangaWithChapter {
				manga: manga.clone(),
				chapter,
			})
			.collect::<Vec<_>>();

		Ok(HomeLayout {
			components: vec![HomeComponent {
				title: Some(String::from("Últimas tiras")),
				subtitle: None,
				value: HomeComponentValue::MangaChapterList {
					page_size: None,
					entries: manga_chapters,
					listing: None,
				},
			}],
		})
	}
}

impl DeepLinkHandler for Latazamediollena {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		if !url.starts_with(BASE_URL) {
			return Ok(None);
		}

		let path = url.trim_start_matches(BASE_URL);
		let segments = path
			.split('/')
			.filter(|part| !part.is_empty())
			.collect::<Vec<_>>();

		match segments.as_slice() {
			["comic", slug, ..] => Ok(Some(DeepLinkResult::Chapter {
				manga_key: String::from(MANGA_KEY),
				key: slug_from_url(slug),
			})),
			_ => Ok(Some(DeepLinkResult::Manga {
				key: String::from(MANGA_KEY),
			})),
		}
	}
}

register_source!(Latazamediollena, Home, DeepLinkHandler);
