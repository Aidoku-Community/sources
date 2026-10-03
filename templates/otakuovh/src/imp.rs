use aidoku::{
	Chapter, DeepLinkResult, FilterValue, HomeLayout, Listing, Manga, MangaPageResult, Page,
	Result,
	alloc::{String, Vec, string::ToString, vec},
	error,
	imports::{canvas::ImageRef, net::Request},
};

use crate::{
	endpoints::Url,
	home::{
		self, load_best_completed, load_editors_choice, load_popular_ongoings, load_recently_added,
	},
	json::ResponseJsonExt,
	models::{
		chapter::{InkBranch, InkChapter},
		common::InkLabel,
		manga::InkManga,
	},
	request::InkRequest,
};

use super::Params;

pub trait Impl {
	fn new() -> Self;

	fn params(&self) -> Params;

	fn get_search_manga_list(
		&self,
		params: &Params,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let binding = (page - 1).to_string();
		let mut search_params: Vec<(String, String)> = vec![
			("strictLabelEqual", "false"),
			("page", binding.as_str()),
			("size", "20"),
		]
		.into_iter()
		.map(|(k, v)| (k.into(), v.into()))
		.collect();
		if let Some(query) = query {
			search_params.push(("search".into(), query))
		}
		filters.into_iter().for_each(|filter| match filter {
			FilterValue::MultiSelect {
				id,
				included,
				excluded: _,
			} => {
				// the API doesn't have exclude parameters, that's why excluded not used.
				if let Some(key) = match id.as_str() {
					"format" => Some("formats"),
					"content_status" => Some("contentStatus"),
					id => Some(id),
				} {
					for value in included {
						search_params.push((key.into(), value));
					}
				}
			}
			FilterValue::Range { id, from, to } => {
				if id == "year" {
					if let Some(min) = from {
						search_params.push(("yearMin".into(), min.to_string()));
					}
					if let Some(max) = to {
						search_params.push(("yearMax".into(), max.to_string()));
					}
				}
				if id == "rating" {
					if let Some(min) = from {
						search_params.push(("averageRatingMin".into(), min.to_string()));
					}
					if let Some(max) = to {
						search_params.push(("averageRatingMax".into(), max.to_string()));
					}
				}
				if id == "chap_count" {
					if let Some(min) = from {
						search_params.push(("chaptersCountMin".into(), min.to_string()));
					}
					if let Some(max) = to {
						search_params.push(("chaptersCountMax".into(), max.to_string()));
					}
				}
			}
			_ => {}
		});

		let url_search = Url::manga_search_with_params(&params.base_url, &search_params);

		let response: Vec<Manga> = Request::get(url_search)?
			.prepared_headers(params)?
			.parse_json::<Vec<InkManga>>()?
			.into_iter()
			.map(|manga| manga.into_basic_manga())
			.collect();

		let has_next_page = !response.is_empty();

		Ok(MangaPageResult {
			entries: response,
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
		let url_branch = Url::manga_branches(&params.base_url, &manga.key, 0);
		let url_chapters = Url::manga_chapters(&params.base_url, &manga.key);

		if needs_details {
			let response_manga = Request::get(Url::manga_details(&params.base_url, &manga.key))?
				.prepared_headers(params)?
				.parse_json::<InkManga>()?;

			manga.clone_from(&response_manga.into_detailed_manga(params.domain.to_string()));
		}

		if needs_chapters {
			let request_branch = Request::get(&url_branch)?
				.prepared_headers(params)?
				.into_request();
			let request_chapters = Request::get(&url_chapters)?
				.prepared_headers(params)?
				.into_request();

			let mut responses = Request::send_all([request_branch, request_chapters]).into_iter();

			let response_branch = responses
				.next()
				.ok_or(error!("Не удалось загрузить данные"))??
				.get_json::<Vec<InkBranch>>()?;

			let response_chapters = responses
				.next()
				.ok_or(error!("Не удалось загрузить данные"))??
				.get_json::<Vec<InkChapter>>()?;

			let chapters = response_chapters
				.into_iter()
				.map(|chapter| chapter.into_chapter(&response_branch))
				.collect();

			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, params: &Params, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url_page = Url::chapter_page(&params.base_url, &chapter.key);
		let response = Request::get(&url_page)?
			.prepared_headers(params)?
			.parse_json::<InkChapter>()?;

		Ok(response
			.pages
			.unwrap_or_default()
			.into_iter()
			.filter_map(|page| page.into_page())
			.collect())
	}

	fn get_manga_list(
		&self,
		params: &Params,
		listing: Listing,
		page: i32,
	) -> Result<MangaPageResult> {
		let search_params: Vec<(String, String)> = vec![
			("strictLabelEqual", "false"),
			("labelsInclude", &listing.id),
			("page", (page - 1).to_string().as_str()),
			("size", "20"),
		]
		.into_iter()
		.map(|(k, v)| (k.into(), v.into()))
		.collect();
		let url_search = Url::manga_search_with_params(&params.base_url, &search_params);

		let response: Vec<Manga> = Request::get(&url_search)?
			.prepared_headers(params)?
			.parse_json::<Vec<InkManga>>()?
			.into_iter()
			.map(|manga| manga.into_basic_manga())
			.collect();
		let has_next_page = response.is_empty();

		Ok(MangaPageResult {
			entries: response,
			has_next_page,
		})
	}

	fn get_home(&self, params: &Params) -> Result<HomeLayout> {
		home::initial_layout();
		load_editors_choice(params)?;
		load_popular_ongoings(params)?;
		load_best_completed(params)?;
		load_recently_added(params)?;

		Ok(HomeLayout::default())
	}

	fn handle_deep_link(&self, _params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url.split('?').min().unwrap_or(&url).trim_end_matches('/');
		let segments: Vec<&str> = path
			.split('/')
			.filter(|segment| !segment.is_empty())
			.collect();

		let marker = segments
			.iter()
			.position(|segment| *segment == "content" || *segment == "genres");

		let Some(marker) = marker else {
			return Ok(None);
		};

		match segments.get(marker) {
			Some(&"content") => {
				let Some(&manga_key) = segments.get(marker + 1) else {
					return Ok(None);
				};

				if let Some(&chapter_key) = segments.get(marker + 2) {
					return Ok(Some(DeepLinkResult::Chapter {
						manga_key: manga_key.to_string(),
						key: chapter_key.to_string(),
					}));
				}

				Ok(Some(DeepLinkResult::Manga {
					key: manga_key.to_string(),
				}))
			}
			Some(_) | None => Ok(None),
		}
	}

	fn process_page_image(
		&self,
		response: aidoku::ImageResponse,
		_context: Option<aidoku::PageContext>,
	) -> Result<ImageRef> {
		let data = response.image.data();
		let binding = self.params();
		let key_bytes = binding.key_decryption.as_bytes();
		let decoded: Vec<u8> = data
			.iter()
			.enumerate()
			.map(|(i, b)| b ^ key_bytes[i % key_bytes.len()])
			.collect();

		Ok(ImageRef::new(&decoded))
	}

	fn get_dynamic_listings(&self) -> Result<Vec<Listing>> {
		let url_label = Url::labels(&self.params().base_url);

		let response = Request::get(&url_label)?
			.prepared_headers(&self.params())?
			.parse_json::<Vec<InkLabel>>()?;

		Ok(response
			.into_iter()
			.map(|label| label.into_listing(aidoku::ListingKind::Default))
			.collect())
	}
}
