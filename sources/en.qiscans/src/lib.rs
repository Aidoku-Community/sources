#![no_std]
use aidoku::{
	Chapter, Manga, Page, PageContent, Result, Source, alloc::vec::Vec, imports::net::Request,
	prelude::*,
};
use iken::{Iken, Impl, Params, models::ChapterResponse};

const BASE_URL: &str = "https://qiscans.org";
const API_URL: &str = "https://api.qiscans.org";

struct QiScans;

impl Impl for QiScans {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			api_url: Some(API_URL.into()),
			get_sort_value: |index| {
				match index {
					0 => "createdAt",
					1 => "updatedAt",
					2 => "totalViews",
					3 => "postTitle",
					_ => "createdAt",
				}
				.into()
			},
			..Default::default()
		}
	}

	fn get_page_list(&self, params: &Params, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!(
			"{}/api/chapter?postId={}&chapterId={}&rsc=1",
			params.get_api_url(),
			manga.key,
			chapter.key
		);
		let mut response = Request::get(url)?
			.header("Referer", &format!("{}/", params.base_url))
			.send()?;
		let data = response.get_json::<ChapterResponse>()?;
		Ok(data
			.chapter
			.images
			.map(|images| {
				images
					.into_iter()
					.map(|image| Page {
						content: PageContent::url(image.url),
						..Default::default()
					})
					.collect()
			})
			.unwrap_or_default())
	}
}

register_source!(Iken<QiScans>, Home, DeepLinkHandler);
