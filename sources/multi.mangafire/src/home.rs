use crate::models::{ApiManga, ApiResponse};
use crate::{BASE_URL, MangaFire};
use aidoku::{
	Chapter, Home, HomeComponent, HomeLayout, HomePartialResult, Manga, MangaWithChapter, Result,
	alloc::vec,
	imports::{
		net::{Request, RequestError, Response},
		std::send_partial_result,
	},
	prelude::*,
};

impl Home for MangaFire {
	fn get_home(&self) -> Result<HomeLayout> {
		// send basic home layout
		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some("Trending".into()),
					subtitle: Some("Trending Now".into()),
					value: aidoku::HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("Latest Updates".into()),
					subtitle: Some("Fresh Chapters".into()),
					value: aidoku::HomeComponentValue::empty_manga_chapter_list(),
				},
			],
		}));

		let responses: [core::result::Result<Response, RequestError>; 2] = Request::send_all([
			Request::get(format!(
				"{BASE_URL}/api/top-titles?type=trending&days=1&limit=30"
			))?,
			Request::get(format!(
				"{BASE_URL}/api/titles?order%5Bchapter_updated_at%5D=desc&hot=1&page=1&limit=30"
			))?,
		])
		.try_into()
		.expect("requests vec length should be 2");
		let [trending_res, latest_res] = responses;

		let components = vec![
			HomeComponent {
				title: Some("Trending".into()),
				subtitle: Some("Trending Now".into()),
				value: aidoku::HomeComponentValue::Scroller {
					entries: trending_res?
						.get_json::<ApiResponse<ApiManga>>()
						.map(|json| {
							json.items
								.into_iter()
								.map(Manga::from)
								.map(Into::into)
								.collect()
						})?,
					listing: None,
				},
			},
			HomeComponent {
				title: Some("Latest Updates".into()),
				subtitle: Some("Fresh Chapters".into()),
				value: aidoku::HomeComponentValue::MangaChapterList {
					page_size: Some(5),
					entries: latest_res?
						.get_json::<ApiResponse<ApiManga>>()
						.map(|json| {
							json.items
								.into_iter()
								.map(|item| {
									let chapter_number = item.latest_chapter;
									MangaWithChapter {
										manga: item.into(),
										chapter: Chapter {
											chapter_number,
											..Default::default()
										},
									}
								})
								.collect()
						})?,
					listing: None,
				},
			},
		];

		Ok(HomeLayout { components })
	}
}
