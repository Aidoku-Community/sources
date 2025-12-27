#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, HashMap, Home, HomeComponent,
	HomeLayout, HomePartialResult, Link, LinkValue, Listing, ListingProvider, Manga,
	MangaPageResult, MangaWithChapter, Page, Result, Source,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::{QueryParameters, encode_uri_component},
	imports::{
		net::{Request, RequestError, Response},
		std::send_partial_result,
	},
	prelude::*,
};

mod helpers;
mod models;
mod settings;

use models::*;

const BASE_URL: &str = "https://comix.to";
const API_URL: &str = "https://comix.to/api/v2";

const NSFW_GENRE_IDS: &[&str] = &["87264", "8", "87265", "13", "87266", "87268"];

struct Comix;

impl Source for Comix {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut qs = QueryParameters::new();
		qs.push("page", Some(&page.to_string()));
		if query.is_some() {
			qs.push("keyword", query.as_deref());
		}

		let mut has_sort_filter = false;

		for filter in filters {
			match filter {
				FilterValue::Text { id, value } => {
					let url = format!(
						"{API_URL}/terms?type={id}&keyword={}&limit=1",
						encode_uri_component(value)
					);
					let id = Request::get(url)?
						.json_owned::<TermResponse>()?
						.result
						.items
						.first()
						.map(|t| t.term_id)
						.ok_or_else(|| error!("No matching {id}s"))?;
					qs.push(&format!("{id}s[]"), Some(&id.to_string()));
				}
				FilterValue::Sort {
					id,
					index,
					ascending,
				} => {
					qs.push(
						&format!(
							"{id}[{}]",
							match index {
								0 => "relevance",
								1 => "chapter_updated_at",
								2 => "created_at",
								3 => "title",
								4 => "year",
								5 => "score",
								6 => "views_7d",
								7 => "views_30d",
								8 => "views_90d",
								9 => "views_total",
								10 => "follows_total",
								_ => "relevance",
							}
						),
						Some(if (index == 3 && !ascending) || (index != 3 && ascending) {
							"asc"
						} else {
							"desc"
						}),
					);
					has_sort_filter = true;
				}
				FilterValue::Select { id, value } => {
					qs.push(&id, Some(&value));
				}
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} => {
					for value in included {
						qs.push(&id, Some(&value));
					}
					for value in excluded {
						qs.push(&id, Some(&format!("-{value}")));
					}
				}
				_ => continue,
			}
		}

		if !has_sort_filter {
			qs.push("order[relevance]", Some("desc"));
		}

		if settings::hide_nsfw() {
			for genre_id in NSFW_GENRE_IDS {
				qs.push("genres[]", Some(&format!("-{genre_id}")));
			}
		}

		let url = format!("{API_URL}/manga?{qs}");
		Request::get(url)?
			.json_owned::<SearchResponse>()
			.map(Into::into)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let url = format!(
				"{API_URL}/manga/{}/?includes[]=demographic\
									&includes[]=genre\
									&includes[]=theme\
									&includes[]=author\
									&includes[]=artist\
									&includes[]=publisher",
				manga.key
			);
			let json: SingleMangaResponse = Request::get(&url)?.json_owned()?;

			manga.copy_from(json.result.into());

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let limit = 100;
			let mut page = 1;
			let deduplicate = settings::get_dedupchapter();
			let mut chapter_map: HashMap<String, ComixChapter> = HashMap::new();
			let mut chapter_list: Vec<ComixChapter> = Vec::new();
			loop {
				let url = format!(
					"{API_URL}/manga/{}/chapters?limit={limit}&page={page}&order[number]=desc",
					manga.key
				);

				let res = Request::get(url)?.json_owned::<ChapterDetailsResponse>()?;

				let items = res.result.items;

				if deduplicate {
					for item in items {
						helpers::dedup_insert(&mut chapter_map, item);
					}
				} else {
					chapter_list.extend(items);
				}

				if res.result.pagination.current_page >= res.result.pagination.last_page {
					break;
				}

				page += 1;
			}

			let mut chapters: Vec<Chapter> = if deduplicate {
				chapter_map
					.into_values()
					.map(|item| item.into_chapter(&manga.key))
					.collect()
			} else {
				chapter_list
					.into_iter()
					.map(|item| item.into_chapter(&manga.key))
					.collect()
			};

			if deduplicate {
				chapters.sort_by(|a, b| {
					b.chapter_number
						.partial_cmp(&a.chapter_number)
						.unwrap_or(core::cmp::Ordering::Equal)
				});
			}

			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{API_URL}/chapters/{}", chapter.key);
		let json: ChapterResponse = Request::get(url)?.json_owned()?;

		let Some(result) = json.result else {
			bail!("Missing chapter")
		};

		Ok(result.images.into_iter().map(Into::into).collect())
	}
}

impl Home for Comix {
	fn get_home(&self) -> Result<HomeLayout> {
		// send basic layout
		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some("Most Recent Popular".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("Most Follows New Comics".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("Latest Updates (Hot)".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("Recently Added".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_manga_chapter_list(),
				},
			],
		}));

		let extra_qs = if settings::hide_nsfw() {
			NSFW_GENRE_IDS
				.iter()
				.map(|id| format!("&genres[]=-{id}"))
				.collect::<String>()
		} else {
			Default::default()
		};

		let responses: [core::result::Result<Response, RequestError>; 4] = Request::send_all([
			// most recent popular
			Request::get(format!(
				"{API_URL}/top?type=trending&days=1&limit=20{extra_qs}"
			))?,
			// most follows new comics
			Request::get(format!(
				"{API_URL}/top?type=follows&days=1&limit=20{extra_qs}"
			))?,
			// latest updates (hot)
			Request::get(format!(
				"{API_URL}/manga?scope=hot&limit=30&order[chapter_updated_at]=desc&page=1{extra_qs}"
			))?,
			// recently added
			Request::get(format!(
				"{API_URL}/manga?order[created_at]=desc&limit=10&page=1{extra_qs}"
			))?,
		])
		.try_into()
		.expect("requests vec length should be 4");

		let [popular_res, follows_res, latest_res, recent_res] = responses;

		for (response, title) in [
			(popular_res, "Most Recent Popular"),
			(follows_res, "Most Follows New Comics"),
			(latest_res, "Latest Updates (Hot)"),
		] {
			let entries = response?
				.get_json::<SearchResponse>()?
				.result
				.items
				.into_iter()
				.map(|m| {
					let manga = Manga::from(m);
					Link {
						title: manga.title.clone(),
						subtitle: None,
						image_url: manga.cover.clone(),
						value: Some(LinkValue::Manga(manga)),
					}
				})
				.collect();
			send_partial_result(&HomePartialResult::Component(HomeComponent {
				title: Some(title.into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::Scroller {
					entries,
					listing: None,
				},
			}));
		}

		{
			let entries = recent_res?
				.get_json::<SearchResponse>()?
				.result
				.items
				.into_iter()
				.map(|m| {
					let chapter_number = m.latest_chapter;
					let date_uploaded = m.chapter_updated_at;
					let manga = Manga::from(m);
					MangaWithChapter {
						manga,
						chapter: Chapter {
							chapter_number,
							date_uploaded,
							..Default::default()
						},
					}
				})
				.collect();
			send_partial_result(&HomePartialResult::Component(HomeComponent {
				title: Some("Recently Added".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::MangaChapterList {
					page_size: None,
					entries,
					listing: None,
				},
			}));
		}

		Ok(HomeLayout::default())
	}
}

impl ListingProvider for Comix {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let trending = |types: Vec<String>| {
			self.get_search_manga_list(
				None,
				page,
				vec![
					FilterValue::Sort {
						id: "order".into(),
						index: 8, // most views 1mo
						ascending: false,
					},
					FilterValue::MultiSelect {
						id: "types[]".into(),
						included: types,
						excluded: Default::default(),
					},
				],
			)
		};

		match listing.id.as_str() {
			"Trending Webtoon" => trending(vec!["manhua".into(), "manhwa".into()]),
			"Trending Manga" => trending(vec!["manga".into()]),
			_ => bail!("Unknown listing"),
		}
	}
}

impl DeepLinkHandler for Comix {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};

		// ex: https://comix.to/title/pvry-one-piece
		// ex: https://comix.to/title/pvry-one-piece/5498414-chapter-1

		let mut segments = path.split('/');

		if let (Some("title"), Some(manga_segment)) = (segments.next(), segments.next()) {
			// ex: pvry-one-piece -> pvry
			let manga_key = manga_segment.split('-').next().unwrap_or(manga_segment);

			if let Some(chapter_segment) = segments.next() {
				// ex: 5498414-chapter-1 -> 5498414
				let chapter_key = chapter_segment.split('-').next().unwrap_or("");
				return Ok(Some(DeepLinkResult::Chapter {
					manga_key: manga_key.to_string(),
					key: chapter_key.to_string(),
				}));
			} else {
				return Ok(Some(DeepLinkResult::Manga {
					key: manga_key.to_string(),
				}));
			}
		}

		Ok(None)
	}
}

register_source!(Comix, Home, ListingProvider, DeepLinkHandler);
