use super::Params;
use crate::models::*;
use aidoku::{
	AidokuError, Chapter, DeepLinkResult, FilterValue, HomeLayout, Manga, MangaPageResult, Page,
	Result,
	alloc::{String, Vec, vec},
	imports::net::Request,
	prelude::*,
};
use chrono::{DateTime, Utc};

pub trait Impl {
	fn new() -> Self;

	fn params(&self) -> Params;

	fn to_manga_status(&self, status: String) -> aidoku::MangaStatus {
		match status.as_str() {
			"En cours" | "On going" => aidoku::MangaStatus::Ongoing,
			"Terminé" | "Completed" => aidoku::MangaStatus::Completed,
			_ => aidoku::MangaStatus::Unknown,
		}
	}

	fn to_manga_content_rating(&self, rating: i32) -> aidoku::ContentRating {
		match rating {
			0 => aidoku::ContentRating::Safe,
			1 => aidoku::ContentRating::NSFW,
			_ => aidoku::ContentRating::Unknown,
		}
	}

	fn to_manga(&self, comic: PizzaComicDto, base_url: &str) -> Manga {
		Manga {
			key: comic.slug,
			title: comic.title,
			description: Some(comic.description),
			url: Some(format!("{}{}", base_url, comic.url)),
			cover: Some(comic.thumbnail),
			authors: Some(vec![comic.author]),
			artists: comic
				.artist
				.filter(|a| !a.is_empty())
				.map(|a| vec![a.clone()]),
			viewer: aidoku::Viewer::RightToLeft,
			content_rating: self.to_manga_content_rating(comic.adult),
			status: self.to_manga_status(comic.status.unwrap_or("".into())),
			..Default::default()
		}
	}

	fn to_mangas(&self, comics: Vec<PizzaComicDto>, base_url: &str) -> Vec<Manga> {
		comics
			.into_iter()
			.map(|comic| self.to_manga(comic, base_url))
			.collect::<Vec<_>>()
	}

	fn get_latest_mangas(&self, base_url: &str) -> Result<Vec<Manga>> {
		let response: PizzaResultsDto =
			Request::get(format!("{}/api/comics", base_url))?.json_owned()?;

		let mut comics = response.comics;
		comics.sort_by(|a, b| {
			let a_date = a
				.last_chapter
				.as_ref()
				.map(|c| c.published_on.as_str())
				.unwrap_or("");

			let b_date = b
				.last_chapter
				.as_ref()
				.map(|c| c.published_on.as_str())
				.unwrap_or("");

			b_date.cmp(a_date)
		});

		Ok(self.to_mangas(comics, base_url))
	}

	fn get_search_manga_list(
		&self,
		params: &Params,
		query: Option<String>,
		_page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let url = if let Some(q) = query {
			if q.len() > 2 {
				format!("{}/api/search/{}", params.base_url, q)
			} else {
				format!("{}/api/comics", params.base_url)
			}
		} else {
			format!("{}/api/comics", params.base_url)
		};
		let response: PizzaResultsDto = Request::get(url)?.json_owned()?;

		Ok(MangaPageResult {
			entries: self.to_mangas(response.comics, &params.base_url),
			has_next_page: false,
		})
	}

	fn get_manga_update(
		&self,
		params: &Params,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let slug = manga.key.trim();
		if slug.is_empty() {
			return Err(AidokuError::message("Manga key is empty"));
		}

		let response: PizzaResultDto =
			Request::get(format!("{}/api/comics/{slug}", params.base_url))?.json_owned()?;
		let comic = response.comic.ok_or_else(|| {
			AidokuError::message(format!(
				"Comic not found with {}/api/comics/{slug}",
				params.base_url
			))
		})?;

		if needs_details {
			manga = self.to_manga(comic.clone(), &params.base_url);
		}

		if needs_chapters {
			let mut chapters: Vec<Chapter> = Vec::new();

			for chapter in comic.chapters {
				let chapter_title = chapter
					.title
					.filter(|t| !t.is_empty())
					.or(Some(chapter.full_title));

				let date_uploaded = DateTime::parse_from_rfc3339(&chapter.published_on)
					.ok()
					.map(|dt| dt.with_timezone(&Utc).timestamp());

				chapters.push(Chapter {
					key: chapter.url,
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

	fn get_page_list(&self, params: &Params, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let chapter_path = chapter.key.trim();
		if chapter_path.is_empty() {
			return Err(AidokuError::message("Chapter key is empty"));
		}

		let response: PizzaReaderDto =
			Request::get(format!("{}/api{}", params.base_url, chapter_path))?.json_owned()?;

		let pages = response
			.chapter
			.map(|chapter| {
				chapter
					.pages
					.into_iter()
					.map(|url| Page {
						content: aidoku::PageContent::Url(url, None),
						..Default::default()
					})
					.collect::<Vec<_>>()
			})
			.unwrap_or_default();

		Ok(pages)
	}

	fn get_home(&self, params: &Params) -> Result<HomeLayout> {
		let entries = self
			.get_latest_mangas(&params.base_url)?
			.into_iter()
			.take(10)
			.map(|manga| manga.into())
			.collect();

		Ok(HomeLayout {
			components: vec![aidoku::HomeComponent {
				title: Some("Last Updated".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::MangaList {
					ranking: false,
					page_size: Some(10),
					entries,
					listing: None,
				},
			}],
		})
	}

	fn handle_deep_link(&self, params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
		if !url.starts_with(params.base_url.as_ref()) {
			return Ok(None);
		}

		if let Some(path) = url.strip_prefix(params.base_url.as_ref()) {
			let path_parts: Vec<&str> = path.trim_matches('/').split('/').collect();
			if path_parts.len() >= 2 && path_parts[0] == "comics" {
				let manga_key = path_parts[1];
				return Ok(Some(DeepLinkResult::Manga {
					key: manga_key.into(),
				}));
			}

			if path_parts.len() >= 4 && path_parts[0] == "read" {
				let manga_key = path_parts[1];
				return Ok(Some(DeepLinkResult::Chapter {
					manga_key: manga_key.into(),
					key: path.into(),
				}));
			}
		}

		Ok(None)
	}
}
