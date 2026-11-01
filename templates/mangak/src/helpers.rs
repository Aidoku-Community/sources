use crate::models::NextData;
use aidoku::{
	ContentRating, MangaStatus, Result, alloc::string::String, imports::net::Request, prelude::*,
};
use serde::de::DeserializeOwned;

pub const SORTS: [Option<&str>; 10] = [
	None, // Best Match sends no sort, as on the site
	Some("latest"),
	Some("newest"),
	Some("popular"),
	Some("rating"),
	Some("views_today"),
	Some("views_7days"),
	Some("views_30days"),
	Some("views"),
	Some("chapters"),
];

pub fn listing_path(id: &str) -> &'static str {
	match id {
		"trending" => "trending/titles?window=week",
		"popular_updates" => "titles/search?sort=bookmarks&window=today",
		"new" => "titles/search?sort=newest",
		"uncensored" => "titles/search?is_uncensored=true",
		"raw" => "titles/search?is_raw=true",
		_ => "titles/search?sort=latest",
	}
}

pub fn next_data<T: DeserializeOwned>(url: &str) -> Result<T> {
	let html = Request::get(url)?.html()?;
	let script = html
		.select_first("script#__NEXT_DATA__")
		.ok_or_else(|| error!("Missing __NEXT_DATA__"))?;
	let json = script
		.data()
		.or_else(|| script.html())
		.ok_or_else(|| error!("Empty __NEXT_DATA__"))?;
	Ok(serde_json::from_str::<NextData<T>>(&json)?.props.page_props)
}

pub fn content_rating(genres: &[String]) -> ContentRating {
	let has = |wanted: &[&str]| genres.iter().any(|genre| wanted.contains(&genre.as_str()));
	if has(&["Adult", "Hentai", "Mature", "Smut"]) {
		ContentRating::NSFW
	} else if has(&["Ecchi"]) {
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	}
}

pub fn manga_status(status: Option<String>) -> MangaStatus {
	match status.unwrap_or_default().to_lowercase().as_str() {
		"ongoing" => MangaStatus::Ongoing,
		"completed" => MangaStatus::Completed,
		"hiatus" => MangaStatus::Hiatus,
		"cancelled" | "dropped" => MangaStatus::Cancelled,
		_ => MangaStatus::Unknown,
	}
}
