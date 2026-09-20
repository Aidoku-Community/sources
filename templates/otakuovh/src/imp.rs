use aidoku::{
	Chapter, DeepLinkResult, FilterValue, HomeLayout, Listing, Manga, MangaPageResult, Page, Result, alloc::{
		String, Vec,
		string::ToString,
		vec,
	}, imports::{canvas::ImageRef, net::Request}, println,
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
		let mut search_params: Vec<(String, String)> = vec![];
		if let Some(query) = query {
			search_params.push(("search".to_string(), query))
		}
		search_params.push(("strictLabelEqual".to_string(), "false".to_string()));
		search_params.push(("page".to_string(), binding));
		search_params.push(("size".to_string(), "20".to_string()));
		filters.iter().for_each(|filter| match filter {
			FilterValue::Text { id: _, value: _ } => {},
			FilterValue::Sort {
				id: _,
				index: _,
				ascending: _ ,
			} => {},
			FilterValue::Check { id: _, value: _ } => {}
			FilterValue::Select { id: _, value: _ } => {}
			FilterValue::MultiSelect {
				id,
				included,
				excluded: _,
			} => {
				// the API doesn't have exclude parameters, that's why excluded not used.
				match id.as_str() {
					"status" => {
						included.iter().for_each(|status| {
							search_params.push(("status".to_string(), status.to_string()))
						});
					},
					"country" => {
						included.iter().for_each(|country| {
							search_params.push(("country".to_string(), country.to_string()));
						});
					},
					"format" => {
						included.iter().for_each(|format| {
							search_params.push(("formats".to_string(), format.to_string()));
						});
					}
					"content_status" => {
						included.iter().for_each(|age_rating| {
							search_params.push(("contentStatus".to_string(), age_rating.to_string()));
						});
					}
					_ => {}
				}
			}
			FilterValue::Range { id, from, to } => {
				if id == "year" {
					if let Some(min) = from {
						search_params.push(("yearMin".to_string(), min.to_string()));
					}
					if let Some(max) = to {
						search_params.push(("yearMax".to_string(), max.to_string()));
					}
				}
				if id == "rating" {
					if let Some(min) = from {
						search_params.push(("averageRatingMin".to_string(), min.to_string()));
					}
					if let Some(max) = to {
						search_params.push(("averageRatingMax".to_string(), max.to_string()));
					}
				}
				if id == "chap_count" {
					if let Some(min) = from {
						search_params.push(("chaptersCountMin".to_string(), min.to_string()));
					}
					if let Some(max) = to {
						search_params.push(("chaptersCountMax".to_string(), max.to_string()));
					}
				}
			}
		});

		let url_search = Url::manga_search_with_params(&params.base_url, search_params);
		println!("{}", url_search);
		let response: Vec<Manga> = Request::get(url_search)?
			.prepared_headers(params)?
			.parse_json::<Vec<InkManga>>()?
			.into_iter()
			.map(|manga| manga.into_basic_manga())
			.collect();

		let has_next_page = response.iter().count() > 0;

		Ok(MangaPageResult {
			entries: response,
			has_next_page,
		})
	}

	fn get_manga_update(
		&self,
		params: &Params,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url_manga = Url::manga_details(&params.base_url, &manga.key);
		let url_branch = Url::manga_branches(&params.base_url, &manga.key, 0);
		let url_chapters = Url::manga_chapters(&params.base_url, &manga.key);

		let response_manga = Request::get(&url_manga)?
			.prepared_headers(params)?
			.parse_json::<InkManga>()?;

		let mut manga = Manga {
			..Default::default()
		};

		if needs_details {
			manga.clone_from(&response_manga.into_detailed_manga(self.params().domain.to_string()));
		} else {
			manga.clone_from(&response_manga.into_basic_manga());
		}

		if needs_chapters {
			let response_branch = Request::get(&url_branch)?
				.prepared_headers(params)?
				.parse_json::<Vec<InkBranch>>()?;
			let response_chapters = Request::get(&url_chapters)?
				.prepared_headers(params)?
				.parse_json::<Vec<InkChapter>>()?;

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
			.prepared_headers(&params)?
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
			("strictLabelEqual".to_string(), "false".to_string()),
			("labelsInclude".to_string(), listing.id),
			("page".to_string(), (page - 1).to_string()),
			("size".to_string(), "20".to_string()),
		];
		let url_search = Url::manga_search_with_params(&params.base_url, search_params);

		let response: Vec<Manga> = Request::get(&url_search)?
			.prepared_headers(params)?
			.parse_json::<Vec<InkManga>>()?
			.into_iter()
			.map(|manga| manga.into_basic_manga())
			.collect();

		Ok(MangaPageResult {
			entries: response.clone(),
			has_next_page: response.into_iter().count() > 0,
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
