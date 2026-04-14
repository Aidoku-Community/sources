#![no_std]
use aidoku::{
	error::Result,
	prelude::*,
	std::{net::Request, String, Vec},
	Chapter, DeepLink, Filter, Listing, Manga, MangaPageResult, Page,
};

mod parser;

const BASE_URL: &str = "https://omegascans.org";
const BASE_API_URL: &str = "https://api.omegascans.org";

#[no_mangle]
pub extern "C" fn abort() -> ! {
	loop {}
}

#[get_manga_list]
fn get_manga_list(filters: Vec<Filter>, page: i32) -> Result<MangaPageResult> {
	parser::parse_manga_list(String::from(BASE_URL), filters, page)
}

#[get_manga_listing]
fn get_manga_listing(listing: Listing, page: i32) -> Result<MangaPageResult> {
	parser::parse_manga_listing(String::from(BASE_URL), listing, page)
}

#[get_manga_details]
fn get_manga_details(manga_id: String) -> Result<Manga> {
	parser::parse_manga_details(&String::from(BASE_URL), manga_id)
}

#[get_chapter_list]
fn get_chapter_list(manga_id: String) -> Result<Vec<Chapter>> {
	parser::parse_chapter_list(String::from(BASE_URL), manga_id)
}

#[get_page_list]
fn get_page_list(manga_id: String, chapter_id: String) -> Result<Vec<Page>> {
	parser::parse_page_list(String::from(BASE_URL), manga_id, chapter_id)
}

#[modify_image_request]
fn modify_image_request(request: Request) {
	parser::modify_image_request(String::from(BASE_URL), request)
}

#[handle_url]
fn handle_url(url: String) -> Result<DeepLink> {
	let parts = url.split('/').filter(|part| !part.is_empty()).collect::<Vec<_>>();

	if parts.len() < 3 || parts[1] != "omegascans.org" || parts[2] != "series" || parts.len() < 4 {
		return Ok(DeepLink::default());
	}

	let manga_id = String::from(parts[3]);
	let manga = get_manga_details(manga_id.clone()).ok();

	let chapter = if parts.len() >= 5 {
		let chapter_id = String::from(parts[4]);
		get_chapter_list(manga_id)
			.ok()
			.and_then(|chapters| chapters.into_iter().find(|chapter| chapter.id == chapter_id))
	} else {
		None
	};

	Ok(DeepLink { manga, chapter })
}
