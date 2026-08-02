use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	imports::{html::Document, net::Request, std::parse_date_with_options},
};

pub const BASE_URL: &str = "https://www.latazamediollena.es";
pub const MANGA_KEY: &str = "es.latazamediollena";
const ARCHIVE_URL: &str = "https://www.latazamediollena.es/comic/";

pub fn comic_info() -> Manga {
	Manga {
		key: String::from(MANGA_KEY),
		title: String::from("La Taza Medio Llena"),
		cover: Some(String::from(
			"https://www.latazamediollena.es/wp-content/uploads/2023/05/cropped-Taza-medio-llena-logo-web_v2_grande_texto-visible.png",
		)),
		authors: Some(vec![String::from("Laurielle")]),
		artists: Some(vec![String::from("Laurielle")]),
		description: Some(String::from(
			"Un cómic de fantasía, tés calentitos junto al fuego... y monstruos.",
		)),
		url: Some(String::from(BASE_URL)),
		status: MangaStatus::Ongoing,
		content_rating: ContentRating::Safe,
		viewer: Viewer::LeftToRight,
		..Default::default()
	}
}

/// Pulls the last non-empty path segment out of a comic url, ignoring any
/// query string (e.g. "https://.../comic/no-es-grave/?sid=27" -> "no-es-grave").
pub fn slug_from_url(url: &str) -> String {
	let without_query = url.split('?').next().unwrap_or(url);
	without_query
		.split('/')
		.rfind(|part| !part.is_empty())
		.unwrap_or_default()
		.into()
}

/// Turns a url slug like "no-es-grave" into a readable title "No es grave".
/// This is only an approximation of the real title (accents are lost), used
/// to avoid an extra request per chapter just to read the page's <h1>.
pub fn title_from_slug(slug: &str) -> String {
	let mut title = slug.replace('-', " ");
	if let Some(first) = title.get_mut(0..1) {
		first.make_ascii_uppercase();
	}
	title
}

/// Parses a Spanish long-form date such as "30 de julio de 2026" into a
/// Unix timestamp.
pub fn parse_spanish_date(text: &str) -> Option<i64> {
	parse_date_with_options(text, "d 'de' MMMM 'de' yyyy", "es_ES", "UTC")
}

/// Fetches a single page of the comic archive (e.g. `/comic/` or
/// `/comic/page/N/`), returning its chapters (newest first) alongside the
/// url of the next (older) archive page, if any.
pub fn fetch_archive_page(url: &str) -> aidoku::Result<(Vec<Chapter>, Option<String>)> {
	let html = Request::get(url)?.html()?;

	let mut chapters = Vec::new();
	if let Some(items) = html.select("#comic-grid > span.comic-thumbnail-wrapper") {
		for item in items {
			let Some(href) = item.select_first("a").and_then(|a| a.attr("abs:href")) else {
				continue;
			};
			let slug = slug_from_url(&href);
			let date_uploaded = item
				.select_first(".posted-on a")
				.and_then(|a| a.text())
				.and_then(|text| parse_spanish_date(&text));

			let title = title_from_slug(&slug);
			chapters.push(Chapter {
				key: slug,
				title: Some(title),
				date_uploaded,
				url: Some(href),
				..Default::default()
			});
		}
	}

	let next_url = html
		.select_first(".nav-links .nav-previous a")
		.and_then(|a| a.attr("abs:href"));

	Ok((chapters, next_url))
}

/// Walks the whole comic archive (from newest to oldest) and returns every
/// chapter with an ascending `chapter_number` (oldest strip = 1).
pub fn fetch_all_chapters() -> aidoku::Result<Vec<Chapter>> {
	let mut chapters = Vec::new();
	let mut next_url = Some(ARCHIVE_URL.to_string());
	while let Some(url) = next_url {
		let (page_chapters, page_next_url) = fetch_archive_page(&url)?;
		chapters.extend(page_chapters);
		next_url = page_next_url;
	}

	let total = chapters.len();
	for (i, chapter) in chapters.iter_mut().enumerate() {
		chapter.chapter_number = Some((total - i) as f32);
	}

	Ok(chapters)
}

/// Extracts the main strip image url from a chapter/strip page's document.
pub fn parse_page_image_url(html: &Document) -> Option<String> {
	html.select_first("#one-comic-option .default-lang img")
		.and_then(|img| img.attr("abs:src"))
}
