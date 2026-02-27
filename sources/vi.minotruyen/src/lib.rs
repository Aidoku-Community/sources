#![no_std]
use core::ops::Deref;

use aidoku::{
	AidokuError, Chapter, FilterValue, Home, HomeComponent, HomeLayout, HomePartialResult,
	ImageResponse, Manga, MangaPageResult, MangaWithChapter, Page, PageContext, PageImageProcessor,
	Result, Source,
	alloc::{String, Vec, borrow::ToOwned, string::ToString, sync::Arc, vec},
	helpers::uri::QueryParameters,
	imports::{canvas::ImageRef, defaults::defaults_get, net::Request, std::send_partial_result},
	prelude::*,
};

mod crypto;
mod drm_tool;
mod env;
mod models;
mod utils;

use models::*;
use spin::Mutex;

use crate::{
	crypto::decrypt_cryptojs_passphrase, drm_tool::DrmToolWasm, env::SECRET_DATA_CHAPTER,
	utils::extract_data_chapter_block,
};

pub const BASE_URL: &str = "https://minotruyenv5.xyz";
pub const API_URL: &str = "https://api.cloudkk.art";

struct MinoTruyen {
	drm_tool: Arc<Mutex<Option<DrmToolWasm>>>,
}

impl Home for MinoTruyen {
	fn get_home(&self) -> Result<HomeLayout> {
		let is_manga = defaults_get::<String>("type").is_some_and(|s| s == "manga");

		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some("Truyện Nổi Bật".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_big_scroller(),
				},
				HomeComponent {
					title: Some("Mới Cập Nhật".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_manga_chapter_list(),
				},
				HomeComponent {
					title: Some("Top".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_scroller(),
				},
			],
		}));

		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some(String::from("Truyện Nổi Bật")),
			subtitle: None,
			value: aidoku::HomeComponentValue::BigScroller {
				entries: Request::get(format!(
					"{}/api/books/top/featured?category={}&take=10",
					API_URL,
					if is_manga { "manga" } else { "comics" }
				))?
				.send()?
				.get_json::<FeaturedRoot>()?
				.data
				.books
				.into_iter()
				.map(|v| v.into())
				.collect::<Vec<_>>(),
				auto_scroll_interval: Some(10.0),
			},
		}));

		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some(String::from("Top")),
			subtitle: None,
			value: aidoku::HomeComponentValue::MangaList {
				entries: Request::get(format!(
					"{}/api/books/side-home?category={}",
					API_URL,
					if is_manga { "manga" } else { "comics" }
				))?
				.send()?
				.get_json::<SideHomeRoot>()?
				.top_books_view
				.into_iter()
				.map(|v| Manga::from(v).into())
				.collect::<Vec<_>>(),
				listing: None,
				ranking: true,
				page_size: Some(3),
			},
		}));

		let html = Request::get(format!(
			"{BASE_URL}/{}",
			if is_manga { "manga" } else { "comics" }
		))?
		.html()?;
		let text = html
			.select("script")
			.and_then(|mut v| {
				v.rfind(|node| {
					node.html()
						.is_some_and(|v| v.contains("self.__next_f.push([1,\"28:[["))
				})
			})
			.and_then(|f| f.html())
			.map(|t| {
				let input = t.replace("\\\"", "\"").replace("\\\\\"", "\\\"");

				input[43..(input.len() - 14)].to_string()
			});

		if let Some(text) = text {
			let json = serde_json::from_str::<FlightRoot<Vec<FlightMutex>>>(&text)?;

			let entries = json
				.children
				.into_iter()
				.filter_map(|v| match v {
					FlightMutex::Arr(arr) => Some(arr.3.children.3.book.into()),
					_ => None,
				})
				.collect::<Vec<MangaWithChapter>>();

			send_partial_result(&HomePartialResult::Component(HomeComponent {
				title: Some(String::from("Mới Cập Nhật")),
				subtitle: None,
				value: aidoku::HomeComponentValue::MangaChapterList {
					entries,
					page_size: Some(4),
					listing: None,
				},
			}));
		}

		Ok(HomeLayout::default())
	}
}

impl Source for MinoTruyen {
	fn new() -> Self {
		Self {
			drm_tool: Arc::new(Mutex::new(None)),
		}
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut qs = QueryParameters::new();

		qs.push("take", "24".into());
		qs.push("page", Some(&page.to_string()));

		let r#type = defaults_get::<String>("type");
		qs.push("category", r#type.as_deref());

		qs.push("q", query.as_deref());

		for filter in filters {
			if let FilterValue::MultiSelect {
				included, excluded, ..
			} = filter
			{
				qs.push("genres", Some(&included.join(",")));
				qs.push("notgenres", Some(&excluded.join(",")));
			}
		}

		let (entries, has_next_page) = Request::get(format!("{API_URL}/api/books?{qs}"))?
			.send()?
			.get_json::<FeaturedBooksData>()
			.map(|res| {
				(
					res.books.into_iter().map(Manga::from).collect(),
					res.count_books.map(|v| v > (page * 24)).unwrap_or_default(),
				)
			})?;

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
		if needs_details {
			let html = Request::get(format!("{BASE_URL}/{}", manga.key.replace("/", "/books/")))?
				.send()?
				.get_html()?;

			let text = html
				.select("script")
				.and_then(|mut f| {
					f.find(|n| n.html().is_some_and(|v| v.contains("\\\"covers\\\":")))
				})
				.and_then(|n| n.html())
				.unwrap_or_default();

			let Some(json) = extract_next_object(&text, Some(3)) else {
				bail!("extract Next error");
			};

			manga.copy_from(serde_json::from_str::<WrapBook>(&json)?.book.into());

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let chapters = Request::get(format!(
				"{API_URL}/api/chapters/{}?order=desc&take=5000",
				manga.key.rsplit('-').next().unwrap_or_default()
			))?
			.send()?
			.get_json::<Chapters>()?
			.chapters
			.into_iter()
			.map(|c| VChapterF::to(c, &manga))
			.collect::<Vec<_>>();

			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let html = Request::get(format!(
			"{BASE_URL}/{}/{}",
			manga.key.replace("/", "/books/"),
			chapter.key
		))?
		.send()?
		.get_html()?;

		let Some(data) = html.select("script").and_then(|f| {
			for n in f {
				let html = n.html().unwrap_or_default();
				if let Some(data) = extract_data_chapter_block(&html) {
					return Some(data);
				}
			}
			None
		}) else {
			bail!("extract data chapter block error");
		};

		let servers = serde_json::from_str::<Vec<ChapterContent>>(
			&decrypt_cryptojs_passphrase(&data, SECRET_DATA_CHAPTER)
				.map_err(|_| error!("decrypt error"))?,
		)?;

		let server = defaults_get::<String>("server").unwrap_or("0".to_owned());
		let first_server = servers
			.first()
			.ok_or(AidokuError::Message("server not found".to_owned()))?;

		let selected = servers
			.iter()
			.find(|s| s.cloud == server)
			.unwrap_or(first_server);

		let pages = selected
			.content
			.iter()
			.map(Page::from)
			.collect::<Vec<Page>>();

		Ok(pages)
	}
}

impl PageImageProcessor for MinoTruyen {
	fn process_page_image(
		&self,
		response: ImageResponse,
		context: Option<PageContext>,
	) -> Result<ImageRef> {
		if context
			.as_ref()
			.map(|v| {
				v.get("drm_data")
					.map(|v| v.deref())
					.unwrap_or_default()
					.is_empty()
			})
			.unwrap_or(true)
		{
			return Ok(response.image);
		}

		let mut drm_tool = self.drm_tool.lock();
		if drm_tool.is_none() {
			let mut tool = DrmToolWasm::new()
				.map_err(|_| AidokuError::Message("drm tool not found".to_owned()))?;

			tool.start()
				.map_err(|_| AidokuError::Message("drm tool start error".to_owned()))?;

			drm_tool.replace(tool);
		}

		let image = drm_tool
			.as_mut()
			.unwrap()
			.render_image(response, context.as_ref())
			.map_err(|_| AidokuError::Message("drm tool render image error".to_owned()))?;

		// render_image(response, context)
		Ok(image)
	}
}

register_source!(MinoTruyen, Home, PageImageProcessor);
