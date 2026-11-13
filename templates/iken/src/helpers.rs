use crate::{
	Params,
	models::{ChaptersResponse, IkenChapter, Post, PostResponse},
};
use aidoku::{
	Result,
	alloc::{String, Vec, collections::BTreeSet},
	helpers::uri::{QueryParameters, decode_uri},
	imports::net::{Request, Response},
	prelude::*,
};
use serde::de::DeserializeOwned;

pub fn check_response(response: Response, url: &str) -> Result<Response> {
	let status = response.status_code();
	if !(200..300).contains(&status) {
		let content_type = response.get_header("Content-Type");
		let prefix = response
			.get_string()
			.ok()
			.map(|s| s.chars().take(240).collect::<String>());
		println!(
			"URL: {url}; HTTP {status}; Content-Type: {content_type:?}; body prefix: {prefix:?}"
		);
		bail!("HTTP {status}");
	}
	Ok(response)
}

pub(crate) fn request(params: &Params, url: &str) -> Result<Request> {
	Ok(Request::get(url)?
		.header("Accept", "application/json")
		.header("Referer", &format!("{}/", params.base_url))
		.timeout(30.))
}

pub fn get_json<T: DeserializeOwned>(params: &Params, path: &str) -> Result<T> {
	let url = format!("{}{path}", params.api_url);
	check_response(request(params, &url)?.send()?, &url)?.get_json_owned()
}

pub fn post(params: &Params, key: &str) -> Result<Option<Post>> {
	let mut qs = QueryParameters::new();
	if key.parse::<i32>().is_ok() {
		qs.push("postId", Some(key));
	} else {
		let slug = decode_uri(
			key.strip_prefix("/series/")
				.unwrap_or(key)
				.split(['?', '#'])
				.next()
				.unwrap_or(key)
				.trim_end_matches('/'),
		);
		qs.push("postSlug", Some(&slug));
	}
	let url = format!("{}/api/post?{qs}", params.api_url);
	let response = request(params, &url)?.send()?;
	if response.status_code() == 404 {
		return Ok(None);
	}
	Ok(Some(
		check_response(response, &url)?
			.get_json_owned::<PostResponse>()?
			.post,
	))
}

pub fn chapters(params: &Params, id: &str) -> Result<Vec<IkenChapter>> {
	let mut chapters: Vec<IkenChapter> = Vec::new();
	let mut ids = BTreeSet::new();
	loop {
		let data: ChaptersResponse = get_json(
			params,
			&format!("/api/chapters?postId={id}&skip={}&take=100", chapters.len()),
		)?;
		let page = data.post.chapters;
		if page.is_empty() && chapters.len() < data.total_chapter_count {
			bail!("Incomplete chapter list");
		}
		if page.iter().any(|chapter| !ids.insert(chapter.id)) {
			bail!("Repeating chapter page");
		}
		chapters.extend(page);
		if chapters.len() >= data.total_chapter_count {
			break;
		}
	}
	Ok(chapters)
}

pub(crate) fn names(text: &str) -> Option<Vec<String>> {
	let mut names: Vec<String> = Vec::new();
	for name in text.split([',', '&']).map(str::trim) {
		if !name.is_empty() && !names.iter().any(|existing| existing == name) {
			names.push(name.into());
		}
	}
	(!names.is_empty()).then_some(names)
}

pub fn display_title(title: &str, novel: bool, label: bool) -> String {
	let title = title.trim();
	let suffix = ["(Novel)", "[Novel]"]
		.into_iter()
		.find(|suffix| title.ends_with(suffix));
	let novel = novel || suffix.is_some();
	let title = suffix.map_or(title, |suffix| title.trim_end_matches(suffix).trim_end());
	if novel && label {
		format!("{title} (Novel)")
	} else {
		title.into()
	}
}

pub fn content_rating(tags: &[String], adult: bool) -> aidoku::ContentRating {
	use aidoku::ContentRating;
	if adult
		|| tags.iter().any(|tag| {
			matches!(
				tag.to_lowercase().as_str(),
				"adult" | "hentai" | "smut" | "yaoi" | "pornographic"
			)
		}) {
		ContentRating::NSFW
	} else if tags
		.iter()
		.any(|tag| matches!(tag.to_lowercase().as_str(), "ecchi" | "mature"))
	{
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	}
}

pub fn viewer(kind: Option<&str>) -> aidoku::Viewer {
	use aidoku::Viewer;
	match kind {
		Some("MANGA") => Viewer::RightToLeft,
		Some(
			"MANHWA"
			| "MANHUA"
			| "WEBTOON"
			| "MANHWA_BL"
			| "MANHUA_BL"
			| "MANHWA_PLUS_19"
			| "BL_SIN_CENSURA"
			| "PLUS_19_SIN_CENSURA",
		) => Viewer::Webtoon,
		_ => Viewer::Unknown,
	}
}
