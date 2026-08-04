use super::{Params, helpers::*, models::*};
use aidoku::{
	Chapter, ContentRating, DeepLinkResult, FilterValue, HomeComponent, HomeComponentValue,
	HomeLayout, Link, Listing, Manga, MangaPageResult, MangaWithChapter, Page, PageContent, Result,
	Viewer,
	alloc::{
		Vec,
		string::{String, ToString},
		vec,
	},
	helpers::uri::{QueryParameters, encode_uri_component},
	imports::{
		net::Request,
		std::{parse_date, send_partial_result},
	},
	prelude::*,
};

pub trait Impl {
	fn new() -> Self;

	fn params(&self) -> Params;

	fn api_get(&self, params: &Params, url: &str) -> Result<Request> {
		Ok(Request::get(url)?
			.header("Origin", &params.base_url)
			.header("Referer", &format!("{}/", params.base_url)))
	}

	fn search_manga_request(
		&self,
		params: &Params,
		page: i32,
		query: Option<String>,
		filters: Vec<FilterValue>,
	) -> Result<Request> {
		let url = match query.as_deref() {
			Some(query) => format!(
				"{}/series/search?q={}&page={page}",
				params.api_url,
				encode_uri_component(query),
			),
			None => {
				let mut qs = QueryParameters::new();
				qs.push("page", Some(&page.to_string()));
				for filter in &filters {
					if let FilterValue::Select { id, value } = filter
						&& !value.is_empty()
					{
						qs.push(id, Some(value));
					}
				}
				format!("{}/series?{qs}", params.api_url)
			}
		};
		self.api_get(params, &url)
	}

	fn get_search_manga_list(
		&self,
		params: &Params,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let resp: ApiList<ApiSeriesItem> = self
			.search_manga_request(params, page, query, filters)?
			.json_owned()?;
		let has_next_page = resp.next.is_some();
		let entries = resp
			.data
			.into_iter()
			.filter(|series| series.series_type.as_deref() != Some("NOVEL"))
			.map(|series| series.into_manga(&params.base_url))
			.collect();

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn get_manga_update(
		&self,
		params: &Params,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let det: ApiSeriesDetail = self
				.api_get(params, &format!("{}/series/{}", params.api_url, manga.key))?
				.json_owned()?;

			manga.title = String::from(det.title.trim());
			manga.cover = if det.cover.is_empty() {
				None
			} else {
				Some(det.cover)
			};
			manga.url = Some(format!("{}/series/{}", params.base_url, det.slug));
			manga.status = parse_status(det.status.as_deref());
			manga.content_rating = ContentRating::Safe;
			manga.viewer = Viewer::Webtoon;

			if let Some(raw_desc) = det.description {
				let desc = strip_html(&raw_desc);
				if !desc.is_empty() {
					manga.description = Some(desc);
				}
			}

			if let Some(author) = det.author.filter(|author| !author.is_empty()) {
				manga.authors = Some(vec![author]);
			}
			if let Some(artist) = det.artist.filter(|artist| !artist.is_empty()) {
				manga.artists = Some(vec![artist]);
			}

			if let Some(genres) = det.genres {
				let tags: Vec<String> = genres
					.into_iter()
					.map(|genre| genre.name.trim().into())
					.collect();
				if !tags.is_empty() {
					manga.tags = Some(tags);
				}
			}

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let mut chapters = Vec::new();
			let mut page = 1;

			loop {
				let resp: ApiList<ApiChapter> = self
					.api_get(
						params,
						&format!(
							"{}/series/{}/chapters?page={page}",
							params.api_url, manga.key
						),
					)?
					.json_owned()?;
				let has_next = resp.next.is_some();

				for chapter in resp.data {
					if !chapter.is_free {
						continue;
					}
					chapters.push(Chapter {
						key: chapter.slug,
						chapter_number: Some(chapter.number as f32),
						title: chapter.title.filter(|title| !title.is_empty()),
						date_uploaded: chapter.created_at.as_deref().and_then(|date| {
							let date = date.split_once('.').map_or(date, |(before, _)| before);
							parse_date(format!("{date}Z"), "yyyy-MM-dd'T'HH:mm:ss'Z'")
						}),
						..Default::default()
					});
				}

				if !has_next {
					break;
				}
				page += 1;
			}

			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, params: &Params, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let det: ApiChapterDetail = self
			.api_get(
				params,
				&format!(
					"{}/series/{}/chapters/{}",
					params.api_url, manga.key, chapter.key
				),
			)?
			.json_owned()?;

		Ok(det
			.images
			.into_iter()
			.map(|image| Page {
				content: PageContent::url(image.url),
				..Default::default()
			})
			.collect())
	}

	fn get_manga_list(
		&self,
		params: &Params,
		listing: Listing,
		page: i32,
	) -> Result<MangaPageResult> {
		let sort = if listing.id == "Latest" {
			"latest"
		} else {
			"popular"
		};
		let resp: ApiList<ApiSeriesItem> = self
			.api_get(
				params,
				&format!("{}/series?page={page}&sort={sort}", params.api_url),
			)?
			.json_owned()?;

		let has_next_page = resp.next.is_some();
		let entries = resp
			.data
			.into_iter()
			.filter(|series| series.series_type.as_deref() != Some("NOVEL"))
			.map(|series| series.into_manga(&params.base_url))
			.collect();

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn get_home(&self, params: &Params) -> Result<HomeLayout> {
		let resp: ApiHomeResponse = self
			.api_get(params, &format!("{}/home", params.api_url))?
			.json_owned()?;

		let filter_novels = |series: &ApiSeriesItem| series.series_type.as_deref() != Some("NOVEL");
		let to_entry = |series: ApiSeriesItem| -> Option<MangaWithChapter> {
			let chapter = series.chapters.first()?;
			let key = chapter.slug.clone();
			let chapter_number = Some(chapter.number as f32);
			Some(MangaWithChapter {
				manga: series.into_manga(&params.base_url),
				chapter: Chapter {
					key,
					chapter_number,
					..Default::default()
				},
			})
		};

		let popular: Vec<Link> = resp
			.popular
			.into_iter()
			.filter(filter_novels)
			.map(|series| series.into_manga(&params.base_url).into())
			.collect();
		let pinned = resp
			.pinned
			.into_iter()
			.filter(filter_novels)
			.filter_map(to_entry)
			.collect();
		let latest = resp
			.new_series
			.into_iter()
			.filter(filter_novels)
			.filter_map(to_entry)
			.collect();

		Ok(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some(String::from("Popular Today")),
					subtitle: None,
					value: HomeComponentValue::Scroller {
						entries: popular,
						listing: None,
					},
				},
				HomeComponent {
					title: Some(String::from("Pinned Series")),
					subtitle: None,
					value: HomeComponentValue::MangaChapterList {
						entries: pinned,
						page_size: None,
						listing: None,
					},
				},
				HomeComponent {
					title: Some(String::from("Latest Updates")),
					subtitle: None,
					value: HomeComponentValue::MangaChapterList {
						entries: latest,
						page_size: None,
						listing: None,
					},
				},
			],
		})
	}

	fn handle_deep_link(&self, params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
		let prefix = format!("{}/series/", params.base_url);
		let Some(rest) = url.strip_prefix(&prefix) else {
			return Ok(None);
		};
		let slug = rest.split('/').next().unwrap_or(rest);
		let slug = slug.split('?').next().unwrap_or(slug);
		let slug = slug.split('#').next().unwrap_or(slug);
		if slug.is_empty() {
			Ok(None)
		} else {
			Ok(Some(DeepLinkResult::Manga {
				key: String::from(slug),
			}))
		}
	}
}
