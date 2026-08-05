use aidoku::{
	Chapter, ContentRating, Manga, Result, Viewer,
	alloc::{String, Vec},
	helpers::uri::encode_uri,
	imports::{html::Document, net::Request, std::parse_date},
	prelude::*,
};

pub const BASE_URL: &str = "https://fandogamia.com/fanternet/";
const ARCHIVE_URL: &str = "https://fandogamia.com/fanternet/archive/";

/// Series known to host explicit adult content. The archive page itself doesn't
/// expose any content rating metadata, so known series are flagged manually.
const NSFW_SERIES: &[&str] = &["oglaf"];

/// Pulls the last non-empty path segment out of a url, ignoring any query string
/// (e.g. "https://.../archive/cursodecocinapa/" -> "cursodecocinapa").
pub fn slug_from_url(url: &str) -> String {
	let without_query = url.split('?').next().unwrap_or(url);
	without_query
		.split('/')
		.rfind(|part| !part.is_empty())
		.unwrap_or_default()
		.into()
}

fn content_rating_for(slug: &str) -> ContentRating {
	if NSFW_SERIES.contains(&slug) {
		ContentRating::NSFW
	} else {
		ContentRating::Unknown
	}
}

/// Fetches the archive page and returns every series listed in it as a manga entry.
///
/// The series list (`.series_div a`, skipping the first "Todos" entry) and the
/// sidebar series covers (`#menu_series a img`) are always rendered in the same
/// order, so they're zipped together instead of matched by url.
pub fn fetch_series_list() -> Result<Vec<Manga>> {
	let html = Request::get(ARCHIVE_URL)?.html()?;

	let mut series_links = html
		.select(".series_div a")
		.ok_or_else(|| error!("Series list not found."))?;
	series_links.next();

	let covers = html
		.select("#menu_series a img")
		.map(|imgs| imgs.collect::<Vec<_>>())
		.unwrap_or_default();

	let mut entries = Vec::new();
	for (link, cover) in series_links.zip(covers) {
		let Some(href) = link.attr("abs:href") else {
			continue;
		};
		let slug = slug_from_url(&href);
		let title = link.text().unwrap_or_default();
		let content_rating = content_rating_for(&slug);

		entries.push(Manga {
			key: slug,
			title,
			cover: cover.attr("abs:src").map(encode_uri),
			url: Some(href),
			content_rating,
			viewer: Viewer::LeftToRight,
			..Default::default()
		});
	}

	Ok(entries)
}

/// Fetches the full chapter list for a series (newest first).
///
/// The per-series archive page shows the most recent strips as image thumbnails
/// (`.thumbs_div > .thumb`, no date attached) followed by every older strip as a
/// dated text link (`.thumbs_div > p`). Together they cover the whole series.
pub fn fetch_series_chapters(slug: &str) -> Result<Vec<Chapter>> {
	let html = Request::get(format!("{ARCHIVE_URL}{slug}/"))?.html()?;

	let mut chapters = Vec::new();

	if let Some(thumbs) = html.select(".thumbs_div > .thumb") {
		for thumb in thumbs {
			let Some(href) = thumb.select_first("a").and_then(|a| a.attr("abs:href")) else {
				continue;
			};
			let title = thumb.select_first("p a").and_then(|a| a.text());
			chapters.push(Chapter {
				key: slug_from_url(&href),
				title,
				url: Some(href),
				..Default::default()
			});
		}
	}

	if let Some(items) = html.select(".thumbs_div > p") {
		for item in items {
			let Some(a) = item.select_first("a") else {
				continue;
			};
			let Some(href) = a.attr("abs:href") else {
				continue;
			};

			let date_text = a.select_first("i").and_then(|i| i.text());
			let date_uploaded = date_text
				.as_deref()
				.and_then(|text| parse_date(text, "yyyy-MM-dd HH:mm:ss"));

			let full_text = a.text().unwrap_or_default();
			let title = match &date_text {
				Some(date) => full_text
					.strip_prefix(&format!("{date} - "))
					.map(String::from)
					.unwrap_or_else(|| full_text.clone()),
				None => full_text.clone(),
			};

			chapters.push(Chapter {
				key: slug_from_url(&href),
				title: Some(title),
				date_uploaded,
				url: Some(href),
				..Default::default()
			});
		}
	}

	let total = chapters.len();
	for (i, chapter) in chapters.iter_mut().enumerate() {
		chapter.chapter_number = Some((total - i) as f32);
	}

	Ok(chapters)
}

/// Extracts the strip image url from a chapter page's document.
///
/// Image file names on this site often contain literal, unencoded spaces
/// (e.g. "Curso de cocina para exdioses - 055.jpg"), so the url is percent-encoded
/// before being returned, otherwise the image request fails.
pub fn parse_page_image_url(html: &Document) -> Option<String> {
	html.select_first(".img_holder img")
		.and_then(|img| img.attr("src"))
		.map(encode_uri)
}
