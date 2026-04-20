use super::Params;
use crate::models::*;
use aidoku::{
	Chapter, DeepLinkResult, Filter, FilterValue, Manga, MangaPageResult, Page, Result,
	alloc::{String, Vec, vec},
	helpers::uri::encode_uri_component,
	imports::{net::Request, std::parse_date},
	prelude::*,
};

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

	fn to_manga_genres(&self, genres: &[PizzaGenreDto]) -> Vec<String> {
		genres
			.iter()
			.flat_map(|g| g.name.trim().split(['-', '/']).map(str::trim))
			.filter(|s| !s.is_empty())
			.map(String::from)
			.collect()
	}

	fn to_manga(&self, comic: PizzaComicDto, base_url: &str) -> Manga {
		let status = self.to_manga_status(comic.status.as_deref().unwrap_or(""));
		let content_rating = self.to_manga_content_rating(comic.adult);
		let genres = self.to_manga_genres(&comic.genres);

		let mut manga: Manga = comic.into();
		manga.url = Some(format!("{}{}", base_url, manga.url.unwrap_or_default()));
		manga.status = status;
		manga.content_rating = content_rating;
		manga.tags = Some(genres);
		manga
	}

	fn to_mangas(&self, comics: Vec<PizzaComicDto>, base_url: &str) -> Vec<Manga> {
		comics
			.into_iter()
			.map(|comic| self.to_manga(comic, base_url))
			.collect::<Vec<_>>()
	}

	fn get_all_mangas(&self, base_url: &str) -> Result<PizzaResultsDto> {
		Request::get(format!("{}/api/comics", base_url))?.json_owned()
	}

	fn get_latest_mangas(&self, base_url: &str) -> Result<Vec<Manga>> {
		let mut comics = self.get_all_mangas(base_url)?.comics;
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

	fn search_mangas(&self, base_url: &str, query: &str) -> Result<PizzaResultsDto> {
		Request::get(format!(
			"{}/api/search/{}",
			base_url,
			encode_uri_component(query)
		))?
		.json_owned()
	}

	fn get_manga_details(&self, base_url: &str, slug: &str) -> Result<PizzaResultDto> {
		Request::get(format!("{base_url}/api/comics/{slug}"))?.json_owned()
	}

	fn get_search_manga_list(
		&self,
		params: &Params,
		query: Option<String>,
		_page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut response: PizzaResultsDto = if let Some(q) = query
			&& q.len() > 2
		{
			self.search_mangas(&params.base_url, &q)?
		} else {
			self.get_all_mangas(&params.base_url)?
		};

		for filter in filters {
			match filter {
				FilterValue::Sort { id, index, .. } if id == "order" => match index {
					1 => {
						response
							.comics
							.sort_by(|a, b| b.title.to_lowercase().cmp(&a.title.to_lowercase()));
					}
					2 => {
						response.comics.sort_by(|a, b| {
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
					}
					3 => {
						response
							.comics
							.sort_by(|a, b| b.rating.total_cmp(&a.rating));
					}
					4 => {
						response.comics.sort_by(|a, b| b.views.cmp(&a.views));
					}
					_ => {}
				},
				FilterValue::Select { id, value, .. } if id == "genre" && value.trim() != "All" => {
					let selected = value.to_lowercase();
					response.comics.retain(|comic| {
						self.to_manga_genres(&comic.genres)
							.into_iter()
							.any(|genre| genre.to_lowercase() == selected)
					});
				}
				_ => {}
			}
		}

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

		let comic = self
			.get_manga_details(&params.base_url, slug)?
			.comic
			.ok_or_else(|| error!("Comic not found with {slug}"))?;

		if needs_chapters {
			manga.chapters = Some(
				comic
					.chapters
					.iter()
					.map(|chapter| Chapter {
						key: chapter.url.clone(),
						title: chapter
							.title
							.clone()
							.filter(|t| !t.is_empty())
							.or(Some(chapter.full_title.clone())),
						chapter_number: chapter.chapter.map(|n| n as f32),
						volume_number: chapter.volume.map(|n| n as f32),
						date_uploaded: parse_date(
							&chapter.published_on,
							"yyyy-MM-dd'T'HH:mm:ss.SSSSSS'Z'",
						),
						..Default::default()
					})
					.collect(),
			);
		}

		if needs_details {
			manga.copy_from(self.to_manga(comic, &params.base_url));
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

	fn get_dynamic_filters(&self, params: &Params) -> Result<Vec<Filter>> {
		let mut genres: Vec<String> = self
			.get_all_mangas(&params.base_url)?
			.comics
			.into_iter()
			.flat_map(|comic| self.to_manga_genres(&comic.genres))
			.filter(|name| !name.is_empty())
			.collect();

		genres.sort_by_key(|a| a.to_lowercase());
		genres.dedup();
		genres.insert(0, "All".into());

		Ok(vec![aidoku::Filter {
			id: "genre".into(),
			hide_from_header: Some(false),
			title: Some("Genre".into()),
			kind: aidoku::FilterKind::Select {
				is_genre: true,
				uses_tag_style: true,
				options: genres.into_iter().map(|s| s.into()).collect(),
				ids: None,
				default: Some("All".into()),
			},
		}])
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
