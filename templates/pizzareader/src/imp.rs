use super::Params;
use crate::models::*;
use aidoku::{
	Chapter, DeepLinkResult, FilterValue, HomeLayout, Manga, MangaPageResult, Page, Result,
	alloc::{String, Vec, vec},
	helpers::uri::encode_uri_component,
	imports::{net::Request, std::parse_date},
	prelude::*,
};

impl From<PizzaComicDto> for Manga {
	fn from(comic: PizzaComicDto) -> Self {
		let PizzaComicDto {
			slug,
			artist,
			author,
			description,
			title,
			thumbnail,
			url,
			..
		} = comic;

		Manga {
			key: slug,
			title,
			description: Some(description),
			url: Some(url),
			cover: Some(thumbnail),
			authors: Some(vec![author]),
			artists: artist.filter(|a| !a.is_empty()).map(|a| vec![a]),
			viewer: aidoku::Viewer::RightToLeft,
			..Default::default()
		}
	}
}

pub trait Impl {
	fn new() -> Self;

	fn params(&self) -> Params;

	fn to_manga_status(&self, status: &str) -> aidoku::MangaStatus {
		match status {
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
		let status = self.to_manga_status(comic.status.as_deref().unwrap_or(""));
		let content_rating = self.to_manga_content_rating(comic.adult);

		let mut manga: Manga = comic.into();
		manga.url = Some(format!("{}{}", base_url, manga.url.unwrap_or_default()));
		manga.status = status;
		manga.content_rating = content_rating;
		manga
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
				format!("{}/api/search/{}", params.base_url, encode_uri_component(q))
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
			bail!("Manga key is empty");
		}

		let response: PizzaResultDto =
			Request::get(format!("{}/api/comics/{slug}", params.base_url))?.json_owned()?;
		let PizzaComicDto {
			chapters,
			slug,
			artist,
			author,
			description,
			title,
			thumbnail,
			url,
			status,
			adult,
			last_chapter,
			genres,
		} = response
			.comic
			.ok_or_else(|| error!("Comic not found with {}/api/comics/{slug}", params.base_url))?;

		if needs_chapters {
			let mut mapped_chapters: Vec<Chapter> = Vec::new();

			for chapter in chapters {
				mapped_chapters.push(Chapter {
					key: chapter.url,
					title: chapter
						.title
						.filter(|t| !t.is_empty())
						.or(Some(chapter.full_title)),
					chapter_number: chapter.chapter.map(|n| n as f32),
					volume_number: chapter.volume.map(|n| n as f32),
					date_uploaded: parse_date(
						&chapter.published_on,
						"yyyy-MM-dd'T'HH:mm:ss.SSSSSS'Z'",
					),
					..Default::default()
				});
			}

			manga.chapters = Some(mapped_chapters);
		}

		if needs_details {
			let mut details = self.to_manga(
				PizzaComicDto {
					chapters: Vec::new(),
					slug,
					artist,
					author,
					description,
					title,
					thumbnail,
					url,
					status,
					adult,
					last_chapter,
					genres,
				},
				&params.base_url,
			);
			if manga.chapters.is_some() {
				details.chapters = manga.chapters
			}
			manga = details;
		}

		Ok(manga)
	}

	fn get_page_list(&self, params: &Params, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let chapter_path = chapter.key.trim();
		if chapter_path.is_empty() {
			bail!("Chapter key is empty");
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
						content: aidoku::PageContent::url(url),
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
		let normalized_url = url.split(['#', '?']).next().unwrap_or(url.as_str());

		let Some(path) = normalized_url.strip_prefix(params.base_url.as_ref()) else {
			return Ok(None);
		};

		let mut parts = path.trim_matches('/').split('/');

		match (parts.next(), parts.next(), parts.next(), parts.next()) {
			(Some("comics"), Some(manga_key), _, _) => Ok(Some(DeepLinkResult::Manga {
				key: manga_key.into(),
			})),
			(Some("read"), Some(manga_key), Some(_), Some(_)) => {
				Ok(Some(DeepLinkResult::Chapter {
					manga_key: manga_key.into(),
					key: path.into(),
				}))
			}
			_ => Ok(None),
		}
	}
}
