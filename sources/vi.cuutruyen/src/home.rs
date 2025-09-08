// a source made by @c0ntens
use crate::CuuTruyen;
use crate::models::*;
use crate::API_URL;
use aidoku::{
	alloc::{string::ToString, vec, String, Vec},
	imports::{
		net::{Response, Request, RequestError},
		std::send_partial_result,
	},
	prelude::*, 
	Chapter, Home, HomeComponent, HomeLayout, HomePartialResult,
	Link, Listing, ListingKind, Manga, MangaWithChapter, Result
};

impl Home for CuuTruyen {
	fn get_home(&self) -> Result<HomeLayout> {
		// send basic home layout
		send_partial_result(&HomePartialResult::Layout(HomeLayout { components: vec![
			HomeComponent {
				title: Some("Gần Đây Nổi Bật".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::empty_big_scroller(),
			},
			HomeComponent {
				title: Some("Mới Cập Nhật".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::empty_manga_chapter_list(),
			},
			HomeComponent {
				title: Some("Top Manga Tuần".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::empty_scroller(),
			},
			HomeComponent {
				title: Some("Top Manga Tháng".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::empty_scroller(),
			},
		]}));

		let responses: [core::result::Result<Response, RequestError>; 3] = Request::send_all([
			Request::get(format!("{API_URL}/home_a"))?,
			// top week
			Request::get(format!("{API_URL}/mangas/top?duration=week&page=1&per_page=24"))?,
			// top month
			Request::get(format!("{API_URL}/mangas/top?duration=month&page=1&per_page=24"))?,
		])
		.try_into()
		.expect("requests vec length should be 3");

		let [home, week, month] = responses;

		// spotlight mangas
		let manga_id = home?.get_json::<CuuResponse<CuuHome>>()?
			.data.spotlight_mangas
			.iter()
			.map(|value| value.id.to_string())
			.collect::<Vec<String>>();

		let manga_res = Request::send_all(manga_id.iter().map(|id| {
			Request::get(format!("{API_URL}/mangas/{}", id)).unwrap()
		}));

		let spotlight_manga = manga_res
			.into_iter()
			.filter_map(|res| {
				Some(res.ok()?
					.get_json::<CuuResponse<CuuMangas>>()
					.unwrap()
					.data.into()
				)
			})
			.collect::<Vec<Manga>>();

		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some(String::from("Gần Đây Nổi Bật")),
			subtitle: None,
			value: aidoku::HomeComponentValue::BigScroller {
				entries: spotlight_manga,
				auto_scroll_interval: Some(10.0),
			},
		}));

		// latest mangas
		let latest_chapter = Request::get(format!("{API_URL}/home_a"))?.send()?.get_json::<CuuResponse<CuuHome>>()?
			.data.new_chapter_mangas
			.into_iter()
			.map(|value| {
				let key = value.chapter_id.to_string();
				let chapter_number = value.number.parse::<f32>().ok();
				let chapter_num = if chapter_number.is_none() {
					Some(String::from("Chương ") + value.number.as_ref())
				} else { None };
				let date_uploaded = chrono::DateTime::parse_from_rfc3339(&value.created_at)
					.ok()
					.map(|d| d.timestamp());

				MangaWithChapter {
					manga: value.into_basic_manga(),
					chapter: Chapter {
						key,
						title: chapter_num,
						chapter_number,
						date_uploaded,
						..Default::default()
					}
				}
			})
			.collect::<Vec<MangaWithChapter>>();

		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some(String::from("Mới Cập Nhật")),
			subtitle: None,
			value: aidoku::HomeComponentValue::MangaChapterList {
				page_size: Some(5),
				entries: latest_chapter,
				listing: Some(Listing {
					id: String::from("latest"),
					name: String::from("Mới Cập Nhật"),
					kind: ListingKind::Default,
				}),
			},
		}));

		// top week
		let weekly = week?.get_json::<CuuSearchResponse<Vec<CuuManga>>>()?
			.data
			.into_iter()
			.map(|value| value.into_basic_manga().into())
			.collect::<Vec<Link>>();

		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some(String::from("Top Manga Tuần")),
			subtitle: None,
			value: aidoku::HomeComponentValue::Scroller {
				entries: weekly,
				listing: Some(Listing {
					id: String::from("week"),
					name: String::from("Top Manga Tuần"),
					kind: ListingKind::Default,
				}),
			},
		}));

		// top month
		let monthly = month?.get_json::<CuuSearchResponse<Vec<CuuManga>>>()?
			.data
			.into_iter()
			.map(|value| value.into_basic_manga().into())
			.collect::<Vec<Link>>();

		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some(String::from("Top Manga Tháng")),
			subtitle: None,
			value: aidoku::HomeComponentValue::Scroller {
				entries: monthly,
				listing: Some(Listing {
					id: String::from("month"),
					name: String::from("Top Manga Tháng"),
					kind: ListingKind::Default,
				}),
			},
		}));

		Ok(HomeLayout::default())
	}
}