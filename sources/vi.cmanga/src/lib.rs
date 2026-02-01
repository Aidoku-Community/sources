#![no_std]
use aidoku::{
	Chapter, FilterValue, Home, HomeComponent, HomeLayout, HomePartialResult, Manga,
	MangaPageResult, Page, PageContent, Result, Source,
	alloc::{String, Vec, borrow::ToOwned, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::{net::Request, std::send_partial_result},
	prelude::*,
};

mod models;

use models::*;

pub const BASE_URL: &str = "https://cmangax12.com";

struct CManga;

impl Home for CManga {
	fn get_home(&self) -> Result<HomeLayout> {
		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some("Truyện Nổi Bật".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_big_scroller(),
				},
				HomeComponent {
					title: Some("VIP".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_manga_chapter_list(),
				},
				HomeComponent {
					title: Some("TMới Cập Nhật".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("Đề Cử".into()),
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
					"{}/api/home_album_list?file=image&sort=update&tag=&type=hot&limit=30&page=1",
					BASE_URL,
				))?
				.send()?
				.get_json::<WrapResponse<MangaResults>>()?
				.data
				.data
				.into_iter()
				.map(|v| v.into())
				.collect::<Vec<_>>(),
				auto_scroll_interval: Some(10.0),
			},
		}));

		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some(String::from("VIP")),
			subtitle: None,
			value: aidoku::HomeComponentValue::MangaChapterList {
				entries: Request::get(format!(
					"{}/api/home_album_list?file=image&type=vip&sort=update&tag=&limit=20&page=1",
					BASE_URL,
				))?
				.send()?
				.get_json::<WrapResponse<MangaResults>>()?
				.data
				.data
				.into_iter()
				.map(|v| v.into())
				.collect::<Vec<_>>(),
				page_size: Some(3),
				listing: None,
			},
		}));

		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some(String::from("Mới Cập Nhật")),
			subtitle: None,
			value: aidoku::HomeComponentValue::Scroller {
				entries: Request::get(format!(
					"{}/api/home_album_list?file=image&type=unique&sort=update&tag=&limit=20&page=1",
					BASE_URL,
				))?
				.send()?
				.get_json::<WrapResponse<MangaResults>>()?
				.data
				.data
				.into_iter()
				.map(|v| Manga::from(v).into())
				.collect::<Vec<_>>(),
				listing: None,
			},
		}));

		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some(String::from("Đề Cử")),
			subtitle: None,
			value: aidoku::HomeComponentValue::Scroller {
				entries: Request::get(format!(
					"{}/api/home_album_list?file=image&type=hot&sort=update&tag=&limit=30&page=1",
					BASE_URL,
				))?
				.send()?
				.get_json::<WrapResponse<MangaResults>>()?
				.data
				.data
				.into_iter()
				.map(|v| Manga::from(v).into())
				.collect::<Vec<_>>(),
				listing: None,
			},
		}));

		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some(String::from("Khoá")),
			subtitle: None,
			value: aidoku::HomeComponentValue::Scroller {
				entries: Request::get(format!(
					"{}/api/home_album_list?file=image&type=new&sort=update&tag=&limit=20&page=1",
					BASE_URL,
				))?
				.send()?
				.get_json::<WrapResponse<MangaResults>>()?
				.data
				.data
				.into_iter()
				.map(|v| Manga::from(v).into())
				.collect::<Vec<_>>(),
				listing: None,
			},
		}));

		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some(String::from("Độc Quyền")),
			subtitle: None,
			value: aidoku::HomeComponentValue::Scroller {
				entries: Request::get(format!(
					"{}/api/home_album_list?file=image&type=done&sort=update&tag=&limit=20&page=1",
					BASE_URL,
				))?
				.send()?
				.get_json::<WrapResponse<MangaResults>>()?
				.data
				.data
				.into_iter()
				.map(|v| Manga::from(v).into())
				.collect::<Vec<_>>(),
				listing: None,
			},
		}));
		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some(String::from("Hot")),
			subtitle: None,
			value: aidoku::HomeComponentValue::Scroller {
				entries: Request::get(format!(
					"{}/api/home_album_top?file=image&type=fire&limit=5",
					BASE_URL,
				))?
				.send()?
				.get_json::<WrapResponse<MangaResults>>()?
				.data
				.data
				.into_iter()
				.map(|v| Manga::from(v).into())
				.collect::<Vec<_>>(),
				listing: None,
			},
		}));
		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some(String::from("Mới")),
			subtitle: None,
			value: aidoku::HomeComponentValue::Scroller {
				entries: Request::get(format!(
					"{}/api/home_album_top?file=image&type=coin&limit=5",
					BASE_URL,
				))?
				.send()?
				.get_json::<WrapResponse<MangaResults>>()?
				.data
				.data
				.into_iter()
				.map(|v| Manga::from(v).into())
				.collect::<Vec<_>>(),
				listing: None,
			},
		}));
		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some(String::from("Hoàn Thành")),
			subtitle: None,
			value: aidoku::HomeComponentValue::Scroller {
				entries: Request::get(format!(
					"{}/api/home_album_list?file=image&sort=update&limit=14&page=1",
					BASE_URL,
				))?
				.send()?
				.get_json::<WrapResponse<MangaResults>>()?
				.data
				.data
				.into_iter()
				.map(|v| Manga::from(v).into())
				.collect::<Vec<_>>(),
				listing: None,
			},
		}));

		Ok(HomeLayout::default())
	}
}

impl Source for CManga {
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

		qs.push("type", "image".into());
		qs.push("page", Some(&page.to_string()));
		qs.push("limit", "20".into());
		qs.push("team", "0".into());

		if let Some(query) = query {
			qs.push("string", Some(&query))
		}

		for filter in filters {
			match filter {
				FilterValue::MultiSelect { included, .. } => {
					qs.push("id", Some(&included.join(",").to_lowercase()))
				}
				FilterValue::Select { id, value } => qs.push(&id, Some(&value)),
				_ => {}
			}
		}

		let (entries, has_next_page) =
			Request::get(format!("{BASE_URL}/api/home_album_list?{qs}"))?
				.send()?
				.get_json::<WrapResponse<MangaResults>>()
				.map(|res| {
					(
						res.data.data.into_iter().map(Manga::from).collect(),
						res.data.total > (page * 20).into(),
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
			manga.copy_from(
				Request::get(format!(
					"{BASE_URL}/api/get_data_by_id?id={}&table=album&data=info",
					manga.key
				))?
				.send()?
				.get_json::<WrapResponse<MangaResult>>()?
				.data
				.info
				.into(),
			);

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let limit = 100;
			let mut page = 1;

			let mut chapters: Vec<Chapter> = vec![];
			loop {
				let url = format!(
					"{BASE_URL}/api/chapter_list?album={}&page={}&limit={}&v=1v16",
					manga.key, page, limit
				);

				let chunk: Vec<Chapter> = Request::get(url)?
					.send()?
					.get_json::<WrapResponse<Vec<MChapter>>>()?
					.data
					.into_iter()
					.map(|c| {
						let id = c.info.id.to_owned();
						let num = c.info.num.to_owned();
						let mut chapter: Chapter = c.into();

						if let Some(ref base) = manga.url {
							chapter.url = Some(format!("{}/chapter-{}-{}", base, id, num));
						}

						chapter
					})
					.collect();

				let count = chunk.len();
				chapters.extend(chunk);

				// Stop when this page contains less than limit → no more pages
				if count < limit {
					break;
				}

				page += 1;
			}

			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let pages = Request::get(format!(
			"{BASE_URL}/api/chapter_image?chapter={}&v=0",
			chapter.key
		))?
		.send()?
		.get_json::<WrapResponse<ChapterImages>>()?
		.data
		.image
		.into_iter()
		.map(|p| Page {
			content: PageContent::url(p),
			..Default::default()
		})
		.collect();

		Ok(pages)
	}
}

register_source!(CManga, Home);
