#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent, HomeComponentValue,
	HomeLayout, ImageRequestProvider, Listing, ListingProvider, Manga, MangaPageResult, Page,
	PageContent, PageContext, PageImageProcessor, Result, Source,
	alloc::{String, Vec, string::ToString, vec},
	imports::{canvas::ImageRef, net::Request},
	prelude::*,
};
use core::cell::RefCell;
use hashbrown::HashSet;

mod helpers;
mod models;
mod settings;

use models::{MangaPlusResponse, Title};

const BASE_URL: &str = "https://mangaplus.shueisha.co.jp";
const WEB_API_URL: &str = "https://jumpg-webapi.tokyo-cdn.com/api";
const MOBILE_API_URL: &str = "https://jumpg-api.tokyo-cdn.com/api";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36";
const ITEMS_PER_PAGE: usize = 20;

#[derive(Default)]
struct MangaPlus {
	directory: RefCell<Vec<Title>>,
}

impl MangaPlus {
	fn request(&self, url: impl AsRef<str>) -> Result<models::SuccessResult> {
		let response = Request::get(url.as_ref())?
			.header("Accept", "application/json, text/plain, */*")
			.header("Accept-Language", "en-US,en;q=0.9")
			.header("Origin", BASE_URL)
			.header("Referer", &format!("{BASE_URL}/"))
			.header("User-Agent", USER_AGENT)
			.header("Session-Token", &helpers::uuid())
			.data()?;
		MangaPlusResponse::decode(&response)?.result_or_error("MANGA Plus API request failed")
	}

	fn parse_directory(&self, page: i32) -> MangaPageResult {
		let directory = self.directory.borrow();
		let entries = directory
			.iter()
			.skip((page as usize - 1) * ITEMS_PER_PAGE)
			.take(ITEMS_PER_PAGE)
			.map(|title| title.clone().into())
			.collect();
		let has_next_page = (page as usize + 1) * ITEMS_PER_PAGE < directory.len();
		MangaPageResult {
			entries,
			has_next_page,
		}
	}
}

impl Source for MangaPlus {
	fn new() -> Self {
		Self::default()
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		if page == 1 {
			let url = || {
				if let Some(query) = query.as_ref() {
					if let Some(title_id) = query.strip_prefix("id:") {
						return format!(
							"{}/title_detailV3?title_id={title_id}&clang=eng{}",
							helpers::get_api_url(),
							helpers::build_auth_params()
						);
					} else if let Some(chapter_id) = query.strip_prefix("chapter-id:") {
						return format!(
							"{WEB_API_URL}/manga_viewer_v3?chapter_id={chapter_id}&split=no&img_quality=low&clang=eng"
						);
					}
				}
				format!(
					"{}/title_list/all_v3?type=serializing&lang=eng&clang=eng{}",
					helpers::get_api_url(),
					helpers::build_auth_params()
				)
			};

			let result = self.request(url())?;

			let languages = settings::get_languages()?;

			if let Some(details) = result.title_detail_view {
				let entries = if details
					.title
					.language()
					.is_none_or(|lang| languages.contains(&lang))
				{
					vec![details.title.into()]
				} else {
					Vec::new()
				};
				return Ok(MangaPageResult {
					entries,
					has_next_page: false,
				});
			}

			if let Some(viewer) = result.manga_viewer {
				let Some(title_id) = viewer.title_id else {
					bail!("Chapter expired");
				};

				let title: Option<Manga> = self
					.get_manga_update(
						Manga {
							key: title_id.to_string(),
							..Default::default()
						},
						true,
						false,
					)
					.ok();

				return Ok(MangaPageResult {
					entries: title.map(|title| vec![title]).unwrap_or_default(),
					has_next_page: false,
				});
			}

			let Some(all_titles) = result.all_titles_view_v3 else {
				bail!("Failed to fetch title list");
			};

			let titles_list = all_titles
				.titles
				.into_iter()
				.map(|entry| entry.title)
				.filter(|title| {
					title
						.language()
						.is_none_or(|lang| languages.contains(&lang))
				})
				.collect::<Vec<_>>();

			*self.directory.borrow_mut() = if let Some(query) = query {
				let query = query.to_lowercase();
				titles_list
					.into_iter()
					.filter(|title| {
						title.name.to_lowercase().contains(&query)
							|| title
								.author
								.as_ref()
								.is_some_and(|a| a.to_lowercase().contains(&query))
					})
					.collect()
			} else {
				titles_list
			};
		}

		Ok(self.parse_directory(page))
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!(
			"{}/title_detailV3?title_id={}&clang=eng{}",
			helpers::get_api_url(),
			manga.key,
			helpers::build_auth_params()
		);
		let result = self.request(&url)?;

		let Some(details) = result.title_detail_view else {
			bail!("Failed to fetch title details");
		};

		if needs_chapters {
			manga.chapters = Some(
				details
					.chapter_list()
					.into_iter()
					.filter(|c| !c.is_expired())
					.cloned()
					.map(|c| c.into())
					.rev()
					.collect(),
			);
		}

		if needs_details {
			manga.copy_from(details.into());
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!(
			"{}/manga_viewer_v3?chapter_id={}&split={}&img_quality={}&clang=eng{}",
			helpers::get_api_url(),
			chapter.key,
			if settings::get_split() { "yes" } else { "no" },
			settings::get_image_quality(),
			helpers::build_auth_params()
		);
		let result = self.request(&url)?;

		let Some(viewer) = result.manga_viewer else {
			bail!("Failed to fetch manga viewer");
		};

		Ok(viewer
			.pages
			.into_iter()
			.filter_map(|page| page.manga_page)
			.map(|page| {
				let mut context = PageContext::new();
				context.insert(
					"view_token".into(),
					viewer.view_token.clone().unwrap_or_default(),
				);
				if let Some(encryption_key) = page.encryption_key {
					context.insert("key".into(), encryption_key);
				}
				Page {
					content: PageContent::url_context(page.image_url, context),
					..Default::default()
				}
			})
			.collect())
	}
}

impl ImageRequestProvider for MangaPlus {
	fn get_image_request(&self, url: String, context: Option<PageContext>) -> Result<Request> {
		let mut request = Request::get(url)?;
		if let Some(view_token) = context.and_then(|context| context.get("view_token").cloned())
			&& !view_token.is_empty()
		{
			request.set_header("Plus-Vw-Token", &view_token);
		}
		Ok(request)
	}
}

impl PageImageProcessor for MangaPlus {
	fn process_page_image(
		&self,
		response: aidoku::ImageResponse,
		context: Option<PageContext>,
	) -> Result<ImageRef> {
		let Some(context) = context else {
			return Ok(response.image);
		};

		let Some(key) = context.get("key") else {
			return Ok(response.image);
		};

		let data = response.image.data();

		let key_stream: core::result::Result<Vec<u8>, core::num::ParseIntError> = key
			.as_bytes()
			.chunks(2)
			.map(|chunk| {
				let s = core::str::from_utf8(chunk).unwrap();
				u8::from_str_radix(s, 16)
			})
			.collect();

		let Ok(key_stream) = key_stream else {
			bail!("Invalid encryption key");
		};

		let decoded: Vec<u8> = data
			.iter()
			.enumerate()
			.map(|(i, &byte)| byte ^ key_stream[i % key_stream.len()])
			.collect();

		Ok(ImageRef::new(&decoded))
	}
}

impl ListingProvider for MangaPlus {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		if page == 1 {
			let languages = settings::get_languages()?;
			let mut seen = HashSet::new();

			*self.directory.borrow_mut() = match listing.id.as_str() {
				"Updates" => self
					.request(format!("{WEB_API_URL}/web/web_homeV4?lang=eng&clang=eng"))?
					.web_home_view
					.ok_or(error!("Failed to fetch home view"))?
					.groups
					.into_iter()
					.flat_map(|group| group.titles)
					.filter_map(models::UpdatedTitle::title)
					.filter(|title| {
						title
							.language()
							.is_none_or(|lang| languages.contains(&lang))
					})
					.filter(|title| seen.insert(title.title_id))
					.collect(),
				"Ranking" => self
					.request(format!(
						"{WEB_API_URL}/title_list/rankingV2?lang=eng&type=hottest&clang=eng"
					))?
					.title_ranking_view
					.ok_or(error!("Failed to fetch ranking"))?
					.ranked_titles
					.into_iter()
					.flat_map(|group| group.titles)
					.filter(|title| {
						title
							.language()
							.is_none_or(|lang| languages.contains(&lang))
					})
					.filter(|title| seen.insert(title.title_id))
					.collect(),
				_ => bail!("Invalid listing"),
			};
		}
		Ok(self.parse_directory(page))
	}
}

impl Home for MangaPlus {
	fn get_home(&self) -> Result<HomeLayout> {
		let languages = settings::get_languages()?;
		let mut components = Vec::new();
		let mut seen = HashSet::new();
		let updates = self
			.request(format!("{WEB_API_URL}/web/web_homeV4?lang=eng&clang=eng"))?
			.web_home_view
			.ok_or(error!("Failed to fetch home view"))?
			.groups
			.into_iter()
			.flat_map(|group| group.titles)
			.filter_map(models::UpdatedTitle::title)
			.filter(|title| {
				title
					.language()
					.is_none_or(|lang| languages.contains(&lang))
			})
			.filter(|title| seen.insert(title.title_id))
			.map(|title| Manga::from(title).into())
			.collect();
		components.push(HomeComponent {
			title: Some("Daily Updates".into()),
			value: HomeComponentValue::Scroller {
				entries: updates,
				listing: Some(Listing {
					id: "Updates".into(),
					name: "Updates".into(),
					..Default::default()
				}),
			},
			..Default::default()
		});

		let mut seen = HashSet::new();
		let ranking = self
			.request(format!(
				"{WEB_API_URL}/title_list/rankingV2?lang=eng&type=hottest&clang=eng"
			))?
			.title_ranking_view
			.ok_or(error!("Failed to fetch ranking"))?
			.ranked_titles
			.into_iter()
			.flat_map(|group| group.titles)
			.filter(|title| {
				title
					.language()
					.is_none_or(|lang| languages.contains(&lang))
			})
			.filter(|title| seen.insert(title.title_id))
			.map(|title| Manga::from(title).into())
			.take(50)
			.collect();
		components.push(HomeComponent {
			title: Some("Hottest".into()),
			value: HomeComponentValue::MangaList {
				ranking: true,
				page_size: Some(10),
				entries: ranking,
				listing: Some(Listing {
					id: "Ranking".into(),
					name: "Hottest".into(),
					..Default::default()
				}),
			},
			..Default::default()
		});
		Ok(HomeLayout { components })
	}
}

impl DeepLinkHandler for MangaPlus {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};

		const TITLE_PATH: &str = "/titles/";

		if let Some(title_id) = path.strip_prefix(TITLE_PATH) {
			// ex: https://mangaplus.shueisha.co.jp/titles/100171
			Ok(Some(DeepLinkResult::Manga {
				key: title_id.into(),
			}))
		} else {
			// ex: https://mangaplus.shueisha.co.jp/viewer/1009921?timestamp=1760385476283
			Ok(None)
		}
	}
}

register_source!(
	MangaPlus,
	ImageRequestProvider,
	PageImageProcessor,
	Home,
	ListingProvider,
	DeepLinkHandler
);
