#![no_std]

use aidoku::alloc::vec;
use aidoku::{
	AidokuError, Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Listing,
	ListingProvider, Manga, MangaPageResult, MangaStatus, Page, PageContent, Result, Source,
	Viewer,
	alloc::{String, Vec, format, string::ToString},
	imports::{html::Document, net::Request},
	prelude::*,
};
use serde_json::Value;

const BASE_URL: &str = "https://okutoon.com";

/// The catalog is server-rendered at this page size.
const PAGE_SIZE: usize = 24;

/// Reader pages sit behind a Cloudflare managed challenge. When the app
/// receives a challenged response it solves the challenge in a hidden
/// webview that is created with the user agent of the original request, so
/// every request needs to use the same mobile browser agent for the stored
/// clearance cookie to stay valid for native requests.
const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1";

const PAGE_ERROR: &str = "Bölüm sayfaları alınamadı.";
const DETAILS_ERROR: &str = "Seri sayfası alınamadı.";

struct OkuToon;

/// Fetch a page of the server-rendered catalog.
/// `kind` is a `/tur` path (with optional genre segment) and `params` are
/// already-encoded `name=value` query pairs appended with `&`.
fn fetch_listing(path: &str, params: Vec<(&str, Option<String>)>) -> Result<MangaPageResult> {
	let mut url = format!("{BASE_URL}{path}");
	let mut query = String::new();
	for (name, value) in &params {
		query.push_str(if query.is_empty() { "?" } else { "&" });
		query.push_str(name);
		if let Some(value) = value {
			query.push('=');
			query.push_str(value);
		}
	}
	url.push_str(&query);
	let document = request_document(&url)?;
	Ok(parse_cards(&document))
}

fn request_document(url: &str) -> Result<Document> {
	Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.html()
		.map_err(|_| AidokuError::Message(DETAILS_ERROR.to_string()))
}

/// Extract catalog cards from a `/tur` page. The page renders at most 24
/// series per page, so a full page implies another page follows.
fn parse_cards(document: &Document) -> MangaPageResult {
	let mut entries = Vec::new();
	if let Some(cards) = document.select("a.series-card") {
		for card in cards {
			let Some(key) = card
				.attr("href")
				.as_deref()
				.map(slug_from_url)
				.filter(|slug| !slug.is_empty())
			else {
				continue;
			};
			let Some(title) = card
				.select_first(".series-card-title")
				.and_then(|e| e.text())
			else {
				continue;
			};
			let cover = card
				.select_first("img")
				.and_then(|img| img.attr("src"))
				.filter(|src| src.starts_with("https://"));
			entries.push(Manga {
				key,
				title,
				cover,
				..Default::default()
			});
		}
	}
	let has_next_page = entries.len() >= PAGE_SIZE;
	MangaPageResult {
		entries,
		has_next_page,
	}
}

fn slug_from_url(url: &str) -> String {
	url.rsplit('/').next().unwrap_or_default().to_string()
}

/// A full series page: details from the JSON-LD block plus the status and
/// type from the header, and the chapter list below. Parsing details and
/// chapters from one document lets `get_manga_update` avoid a second
/// request when both are needed.
struct SeriesPage {
	manga: Manga,
	chapters: Vec<Chapter>,
}

fn parse_series(key: &str, document: &Document) -> SeriesPage {
	let mut title = String::new();
	let mut cover = None;
	let mut description = None;
	let mut authors = None;
	let mut tags = None;

	if let Some(ld) = document
		.select("script[type=\"application/ld+json\"]")
		.and_then(|list| list.first())
	{
		let json = ld.html().unwrap_or_default();
		// strip the wrapper declaration if present; the block is otherwise
		// pure JSON
		let json = json.trim_start_matches("<script").trim_start();
		if let Ok(value) = serde_json::from_str::<Value>(json)
			&& let Some(series) = value
				.get("@graph")
				.and_then(Value::as_array)
				.and_then(|list| {
					list.iter().find(|node| {
						node.get("@type").and_then(Value::as_str) == Some("ComicSeries")
					})
				}) {
			title = string_field(series, "name");
			cover = string_field(series, "image").into();
			description = string_field(series, "description").into();
			if let Some(author) = series.get("author")
				&& let Some(name) = author.get("name").and_then(Value::as_str)
			{
				authors = Some(vec![name.to_string()]);
			}
			if let Some(genres) = series.get("genre").and_then(Value::as_array) {
				tags = Some(
					genres
						.iter()
						.filter_map(Value::as_str)
						.map(ToString::to_string)
						.collect::<Vec<_>>(),
				);
			}
		}
	}

	if title.is_empty() {
		title = document
			.select_first("h1")
			.and_then(|e| e.text())
			.unwrap_or_default();
		if title.is_empty() {
			title = key.to_string();
		}
	}
	if cover.is_none() {
		cover = document
			.select_first("meta[property=\"og:image\"]")
			.and_then(|m| m.attr("content"));
	}

	// the header kicker holds the series type and publishing status:
	// "<type> · <status>"
	let mut viewer = Viewer::Webtoon;
	let mut status = MangaStatus::Unknown;
	if let Some(kicker) = document.select_first(".series-detail-kicker") {
		let text = kicker.text().unwrap_or_default();
		for part in text.split('·').map(str::trim) {
			match part.to_ascii_lowercase().as_str() {
				"manga" => viewer = Viewer::RightToLeft,
				"devam ediyor" => {
					status = MangaStatus::Ongoing;
				}
				"tamamlandı" => status = MangaStatus::Completed,
				"ara verildi" => status = MangaStatus::Hiatus,
				"bırakıldı" | "terk edildi" => status = MangaStatus::Cancelled,
				_ => {}
			}
		}
	}

	let mut content_rating = ContentRating::Safe;
	if let Some(tags) = tags.as_ref() {
		let lowercase = tags
			.iter()
			.map(|tag| tag.to_ascii_lowercase())
			.collect::<Vec<_>>();
		if lowercase
			.iter()
			.any(|tag| tag.contains("ecchi") || tag.contains("eroti"))
		{
			content_rating = ContentRating::Suggestive;
		}
	}

	let chapters = parse_chapters(key, document);

	SeriesPage {
		manga: Manga {
			key: key.to_string(),
			title,
			cover,
			authors,
			description,
			url: Some(format!("{BASE_URL}/{key}")),
			tags,
			status,
			content_rating,
			viewer,
			..Default::default()
		},
		chapters,
	}
}

fn string_field(value: &Value, field: &str) -> String {
	value
		.get(field)
		.and_then(Value::as_str)
		.unwrap_or_default()
		.to_string()
}

/// Chapters are server-rendered as `div.chapter-item` elements with the
/// chapter number in a data attribute and the slug in the link href,
/// newest first (which the app expects).
fn parse_chapters(key: &str, document: &Document) -> Vec<Chapter> {
	let mut chapters = Vec::new();
	let Some(items) = document.select("div.chapter-item") else {
		return chapters;
	};
	for item in items {
		let Some(chapter_url) = item
			.select_first("a.chapter-item-link")
			.and_then(|a| a.attr("href"))
		else {
			continue;
		};
		let chapter_key = slug_from_url(&chapter_url);
		if chapter_key.is_empty() {
			continue;
		}
		let chapter_url = format!("{BASE_URL}/{key}/{chapter_key}");
		let number = item
			.attr("data-chapter-number")
			.and_then(|n| n.parse::<f32>().ok());
		let title = item
			.select_first(".chapter-title")
			.and_then(|e| e.text())
			.filter(|t| !t.is_empty());
		let date_uploaded = item
			.select_first(".chapter-date")
			.and_then(|e| e.text())
			.and_then(|date| parse_date(&date));
		chapters.push(Chapter {
			url: Some(chapter_url),
			key: chapter_key,
			title,
			chapter_number: number,
			date_uploaded,
			..Default::default()
		});
	}
	chapters
}

/// Parse a Turkish day-month-year date like "3 Kasım 2026" into a unix
/// timestamp at noon UTC.
fn parse_date(text: &str) -> Option<i64> {
	let mut parts = text.split_whitespace();
	let day = parts.next()?.parse::<i64>().ok()?;
	let month = month_index(parts.next()?.to_ascii_lowercase().as_str())?;
	let year = parts.next()?.parse::<i64>().ok()?;
	Some(epoch_from_ymd(year, month, day) + 43_200)
}

fn month_index(name: &str) -> Option<i64> {
	Some(match name {
		"ocak" => 1,
		"subat" | "şubat" => 2,
		"mart" => 3,
		"nisan" => 4,
		"mayis" | "mayıs" => 5,
		"haziran" => 6,
		"temmuz" => 7,
		"agustos" | "ağustos" => 8,
		"eylul" | "eylül" => 9,
		"ekim" => 10,
		"kasim" | "kasım" => 11,
		"aralik" | "aralık" => 12,
		_ => return None,
	})
}

/// Days from 1970-01-01 for a proleptic Gregorian date.
/// http://howardhinnant.github.io/date_algorithms.html#days_from_civil
fn epoch_from_ymd(year: i64, month: i64, day: i64) -> i64 {
	let y = if month <= 2 { year - 1 } else { year };
	let era = if y >= 0 { y } else { y - 399 } / 400;
	let yoe = y - era * 400; // [0, 399]
	let mp = (month + 9) % 12; // [0, 11]
	let doy = (153 * mp + 2) / 5 + day - 1; // [0, 365]
	let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
	(era * 146_097 + doe - 719_468) * 86_400
}

/// Reader pages are server-rendered `img.reader-page` elements whose src
/// attributes are the page images, in order. The image CDN is open, but
/// the reader page itself can be served behind a Cloudflare challenge,
/// which the app resolves transparently using `USER_AGENT`.
fn parse_pages(document: &Document) -> Result<Vec<Page>> {
	let mut pages = Vec::new();
	let images = document
		.select("img.reader-page")
		.ok_or_else(|| AidokuError::Message(PAGE_ERROR.to_string()))?;
	for image in images {
		let Some(url) = image.attr("src").filter(|src| src.starts_with("https://")) else {
			continue;
		};
		pages.push(Page {
			content: PageContent::Url(url, None),
			..Default::default()
		});
	}
	if pages.is_empty() {
		return Err(AidokuError::Message(PAGE_ERROR.to_string()));
	}
	Ok(pages)
}

impl Source for OkuToon {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let query = query.unwrap_or_default();
		let mut genre = "";
		let mut sort = "newest";
		let mut period = "";
		for filter in &filters {
			if let FilterValue::Select { id, value } = filter {
				match id.as_str() {
					"genre" => genre = value.as_str(),
					"sort" => sort = value.as_str(),
					"period" => period = value.as_str(),
					_ => {}
				}
			}
		}

		let path = if genre.is_empty() {
			"/tur".to_string()
		} else {
			format!("/tur/{genre}")
		};
		let encoded_query = encode_query(&query);
		let mut params = Vec::new();
		if !encoded_query.is_empty() {
			params.push(("q", Some(encoded_query)));
		}
		params.push(("sira", Some(sort.to_string())));
		if sort == "popular" && !period.is_empty() {
			params.push(("donem", Some(period.to_string())));
		}
		if page > 1 {
			params.push(("sayfa", Some(page.to_string())));
		}
		fetch_listing(&path, params)
	}

	fn get_manga_update(
		&self,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let key = manga.key.clone();
		let document = request_document(&format!("{BASE_URL}/{key}"))?;
		let series = parse_series(&key, &document);
		let mut manga = manga;
		if needs_details {
			manga.copy_from(series.manga);
		}
		if needs_chapters {
			manga.chapters = Some(series.chapters);
		}
		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let document = request_document(&format!("{}/{}/{}", BASE_URL, manga.key, chapter.key))?;
		parse_pages(&document)
	}
}

impl ListingProvider for OkuToon {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let sort = match listing.id.as_str() {
			"popular" => "popular",
			"updated" => "updated",
			"rating" => "rating",
			_ => "newest",
		};
		let mut params = Vec::new();
		params.push(("sira", Some(sort.to_string())));
		if sort == "popular" {
			params.push(("donem", Some("weekly".to_string())));
		}
		if page > 1 {
			params.push(("sayfa", Some(page.to_string())));
		}
		fetch_listing("/tur", params)
	}
}

impl DeepLinkHandler for OkuToon {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url
			.strip_prefix(BASE_URL)
			.unwrap_or(url.as_str())
			.trim_start_matches('/');
		let mut segments = path.split('/');
		let Some(series) = segments.next().filter(|s| !s.is_empty()) else {
			return Ok(None);
		};
		if series == "tur" {
			return Ok(None);
		}
		Ok(Some(match segments.next() {
			Some(chapter) if chapter.starts_with("bolum") => DeepLinkResult::Chapter {
				manga_key: series.to_string(),
				key: chapter.to_string(),
			},
			_ => DeepLinkResult::Manga {
				key: series.to_string(),
			},
		}))
	}
}

/// Percent-encode a search query for use as a URL parameter.
fn encode_query(query: &str) -> String {
	let mut encoded = String::new();
	for byte in query.bytes() {
		match byte {
			b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
				encoded.push(byte as char)
			}
			_ => encoded.push_str(&format!("%{byte:02X}")),
		}
	}
	encoded
}

register_source!(OkuToon, ListingProvider, DeepLinkHandler);

#[cfg(test)]
mod tests;
