#![no_std]

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider, Manga,
	MangaPageResult, Page, Result, Source,
	alloc::{String, Vec},
	prelude::*,
};

mod crypto;
mod home;
mod json;
mod net;
mod settings;

struct Manhuabika;

impl Source for Manhuabika {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let url = net::Url::from_query_or_filters(query.as_deref(), page, &filters)?;
		let response = url.request()?.json_owned::<json::ExploreResponse>()?;

		Ok(response.data.into())
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let url = net::Url::Manga {
				id: manga.key.clone(),
			};
			let response: json::ComicResponse = url.request()?.json_owned()?;
			let comic: Manga = response.data.comic.into();
			manga = Manga {
				chapters: manga.chapters,
				..comic
			};
		}

		if needs_chapters {
			let mut page = 1;
			let url = net::Url::ChapterList {
				id: manga.key.clone(),
				page,
			};
			let response: json::ChapterResponse = url.request()?.json_owned()?;
			let mut chapters: Vec<aidoku::Chapter> =
				response.data.eps.docs.into_iter().map(Into::into).collect();

			let pages = response.data.eps.pages;
			while page < pages {
				page += 1;
				let next_chapters = get_chapter_list_by_page(manga.key.clone(), page)?;
				chapters.extend(next_chapters);
			}

			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let mut page = 1;
		let url = net::Url::PageList {
			manga_id: manga.key.clone(),
			chapter_id: chapter.key.clone(),
			page,
		};
		let response: json::PageResponse = url.request()?.json_owned()?;
		let mut pages: Vec<Page> = response
			.data
			.pages
			.docs
			.into_iter()
			.map(Into::into)
			.collect();

		let total_pages = response.data.pages.pages;
		let limit = response.data.pages.limit;
		let mut offset = limit;

		while page < total_pages {
			page += 1;
			let next_pages =
				get_page_list_by_page(manga.key.clone(), chapter.key.clone(), page, offset)?;
			pages.extend(next_pages);
			offset += limit;
		}

		Ok(pages)
	}
}

fn get_chapter_list_by_page(id: String, page: i32) -> Result<Vec<aidoku::Chapter>> {
	let url = net::Url::ChapterList { id, page };
	let response: json::ChapterResponse = url.request()?.json_owned()?;
	Ok(response.data.eps.docs.into_iter().map(Into::into).collect())
}

fn get_page_list_by_page(
	manga_id: String,
	chapter_id: String,
	page: i32,
	_offset: i32,
) -> Result<Vec<Page>> {
	let url = net::Url::PageList {
		manga_id,
		chapter_id,
		page,
	};
	let response: json::PageResponse = url.request()?.json_owned()?;
	Ok(response
		.data
		.pages
		.docs
		.into_iter()
		.map(Into::into)
		.collect())
}

impl ListingProvider for Manhuabika {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let mut rank_time = String::new();
		let mut is_random = false;
		let mut is_favourite = false;
		let mut category = String::new();
		let sort = String::from("dd");

		match listing.id.as_str() {
			"dayup" => rank_time = String::from("H24"),
			"weekup" => rank_time = String::from("D7"),
			"monthup" => rank_time = String::from("D30"),
			"random" => is_random = true,
			"favourite" => is_favourite = true,
			"dswj" => category = String::from("大濕推薦"),
			"nndtn" => category = String::from("那年今天"),
			"djkz" => category = String::from("大家都在看"),
			"gfdjkz" => category = String::from("官方都在看"),
			_ => return self.get_search_manga_list(None, page, Vec::new()),
		};

		if !rank_time.is_empty() {
			let url = net::Url::Rank { time: rank_time };
			let response: json::RankResponse = url.request()?.json_owned()?;
			Ok(response.data.into())
		} else if is_random {
			let url = net::Url::Random;
			let response: json::RankResponse = url.request()?.json_owned()?;
			Ok(response.data.into())
		} else if is_favourite {
			let url = net::Url::Favourite { sort, page };
			let response: json::ExploreResponse = url.request()?.json_owned()?;
			Ok(response.data.into())
		} else if !category.is_empty() {
			let url = net::Url::Explore {
				category,
				sort,
				page,
			};
			let response: json::ExploreResponse = url.request()?.json_owned()?;
			Ok(response.data.into())
		} else {
			self.get_search_manga_list(None, page, Vec::new())
		}
	}
}

impl DeepLinkHandler for Manhuabika {
	fn handle_deep_link(&self, _url: String) -> Result<Option<DeepLinkResult>> {
		Ok(None)
	}
}

register_source!(Manhuabika, Home, ListingProvider, DeepLinkHandler);
