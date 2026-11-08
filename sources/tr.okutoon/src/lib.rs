#![no_std]

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

/// Reader pages sit behind a Cloudflare managed challenge. When the app
/// receives a challenged response it solves the challenge in a hidden
/// webview that is created with the user agent of the original request, so
/// every request needs to use the same mobile browser agent for the stored
/// clearance cookie to stay valid for native requests.
const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1";

const PAGE_ERROR: &str = "Bölüm sayfaları alınamadı.";
const NETWORK_ERROR: &str = "Sayfa okutoon.com'dan alınamadı.";

/// Top-level site paths that aren't series slugs.
const NON_SERIES_PATHS: &[&str] = &[
	"tur",
	"arama",
	"giris",
	"kayit",
	"profil",
	"kutuphane",
	"hakkimizda",
	"iletisim",
	"gizlilik",
	"api",
];

struct OkuToon;

/// Fetch a page of the server-rendered catalog.
/// `path` is a `/tur` path (with optional genre segment) and `params` are
/// already-encoded query pairs, joined as `name=value` with `&`.
fn fetch_listing(path: &str, params: Vec<(&str, String)>) -> Result<MangaPageResult> {
	let mut url = format!("{BASE_URL}{path}");
	for (i, (name, value)) in params.iter().enumerate() {
		url.push(if i == 0 { '?' } else { '&' });
		url.push_str(name);
		url.push('=');
		url.push_str(value);
	}
	let document = request_document(&url)?;
	Ok(parse_cards(&document))
}

fn request_document(url: &str) -> Result<Document> {
	Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.html()
		.map_err(|_| AidokuError::Message(NETWORK_ERROR.to_string()))
}

/// Extract catalog cards from a `/tur` page. Multi-page listings render a
/// "Sonraki" (`rel="next"`) button; its presence marks another page.
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
			let cover = card.select_first("img").and_then(|img| image_url(&img));
			entries.push(Manga {
				key,
				title,
				cover,
				..Default::default()
			});
		}
	}
	let has_next_page = document.select_first("a[rel=\"next\"]").is_some();
	MangaPageResult {
		entries,
		has_next_page,
	}
}

fn slug_from_url(url: &str) -> String {
	url.rsplit('/').next().unwrap_or_default().to_string()
}

/// Parse series details from the JSON-LD block plus the type and status
/// from the page header.
fn parse_details(key: &str, document: &Document) -> Manga {
	let mut title = String::new();
	let mut cover = None;
	let mut description = None;
	let mut authors = None;
	let mut tags = None;

	if let Some(ld) = document
		.select("script[type=\"application/ld+json\"]")
		.and_then(|list| list.first())
	{
		// script contents are raw data; fall back to inner html where data
		// isn't available (the test runner doesn't implement it)
		let json = ld.data().or_else(|| ld.html()).unwrap_or_default();
		if let Ok(value) = serde_json::from_str::<Value>(json.trim())
			&& let Some(series) = value
				.get("@graph")
				.and_then(Value::as_array)
				.and_then(|list| {
					list.iter().find(|node| {
						node.get("@type").and_then(Value::as_str) == Some("ComicSeries")
					})
				}) {
			title = string_field(series, "name").unwrap_or_default();
			cover = string_field(series, "image");
			description = string_field(series, "description");
			// `author` is either a single Person or an array of them
			let names = match series.get("author") {
				Some(Value::Array(list)) => list
					.iter()
					.filter_map(|a| string_field(a, "name"))
					.collect(),
				Some(author) => string_field(author, "name").into_iter().collect(),
				None => Vec::new(),
			};
			if !names.is_empty() {
				authors = Some(names);
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
			match fold_turkish(part).as_str() {
				"manga" => viewer = Viewer::RightToLeft,
				"devam ediyor" => status = MangaStatus::Ongoing,
				"tamamlandi" => status = MangaStatus::Completed,
				"ara verildi" => status = MangaStatus::Hiatus,
				"birakildi" | "terk edildi" => status = MangaStatus::Cancelled,
				_ => {}
			}
		}
	}

	let suggestive = tags.iter().flatten().any(|tag| {
		let tag = tag.to_lowercase();
		tag.contains("ecchi") || tag.contains("eroti")
	});
	let content_rating = if suggestive {
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	};

	Manga {
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
	}
}

/// A non-empty string field of a JSON object.
fn string_field(value: &Value, field: &str) -> Option<String> {
	value
		.get(field)
		.and_then(Value::as_str)
		.map(str::trim)
		.filter(|s| !s.is_empty())
		.map(ToString::to_string)
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
		// chapter titles always mirror the chapter number ("Bölüm 366");
		// only keep a title when the site has a real one
		let label = item.attr("data-chapter-number-label");
		let title = item
			.select_first(".chapter-title")
			.and_then(|e| e.text())
			.filter(|t| !t.is_empty() && t.strip_prefix("Bölüm ") != label.as_deref());
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
	let month = month_index(fold_turkish(parts.next()?).as_str())?;
	let year = parts.next()?.parse::<i64>().ok()?;
	Some(epoch_from_ymd(year, month, day) + 43_200)
}

fn month_index(name: &str) -> Option<i64> {
	Some(match name {
		"ocak" => 1,
		"subat" => 2,
		"mart" => 3,
		"nisan" => 4,
		"mayis" => 5,
		"haziran" => 6,
		"temmuz" => 7,
		"agustos" => 8,
		"eylul" => 9,
		"ekim" => 10,
		"kasim" => 11,
		"aralik" => 12,
		_ => return None,
	})
}

/// Lowercase and strip Turkish diacritics so "TAMAMLANDI", "Tamamlandı" and
/// "tamamlandi" all compare equal. Plain `to_lowercase` maps "I" to "i" and
/// "İ" to "i\u{307}", so both dotted and dotless forms are folded to "i".
fn fold_turkish(text: &str) -> String {
	text.chars()
		.flat_map(char::to_lowercase)
		.filter_map(|c| match c {
			'\u{307}' => None,
			'ı' => Some('i'),
			'ş' => Some('s'),
			'ğ' => Some('g'),
			'ü' => Some('u'),
			'ö' => Some('o'),
			'ç' => Some('c'),
			c => Some(c),
		})
		.collect()
}

/// Resolve an image attribute to an absolute https URL.
fn absolute_url(src: &str) -> Option<String> {
	let src = src.trim();
	if src.starts_with("https://") {
		Some(src.to_string())
	} else if let Some(rest) = src.strip_prefix("//") {
		Some(format!("https://{rest}"))
	} else if src.starts_with('/') {
		Some(format!("{BASE_URL}{src}"))
	} else {
		None
	}
}

/// The real image URL of an `img`, preferring lazy-load attributes over a
/// possible placeholder `src`.
fn image_url(img: &aidoku::imports::html::Element) -> Option<String> {
	["data-src", "data-lazy-src", "src"]
		.iter()
		.filter_map(|attr| img.attr(attr))
		.find_map(|src| absolute_url(&src))
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
		let Some(url) = image_url(&image) else {
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
			params.push(("q", encoded_query));
		}
		params.push(("sira", sort.to_string()));
		if sort == "popular" && !period.is_empty() {
			params.push(("donem", period.to_string()));
		}
		if page > 1 {
			params.push(("sayfa", page.to_string()));
		}
		fetch_listing(&path, params)
	}

	fn get_manga_update(
		&self,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if !needs_details && !needs_chapters {
			return Ok(manga);
		}
		let key = manga.key.clone();
		let document = request_document(&format!("{BASE_URL}/{key}"))?;
		let mut manga = manga;
		if needs_details {
			manga.copy_from(parse_details(&key, &document));
		}
		if needs_chapters {
			manga.chapters = Some(parse_chapters(&key, &document));
		}
		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		// network failures keep NETWORK_ERROR; PAGE_ERROR is for pages that
		// load but contain no images
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
		params.push(("sira", sort.to_string()));
		if sort == "popular" {
			params.push(("donem", "weekly".to_string()));
		}
		if page > 1 {
			params.push(("sayfa", page.to_string()));
		}
		fetch_listing("/tur", params)
	}
}

impl DeepLinkHandler for OkuToon {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		// accept http/https and an optional www. host; drop query/fragment
		let rest = url
			.strip_prefix("https://")
			.or_else(|| url.strip_prefix("http://"))
			.unwrap_or(url.as_str());
		let rest = rest.strip_prefix("www.").unwrap_or(rest);
		// the host must end exactly here, not continue as `okutoon.com.evil`
		let Some(path) = rest
			.strip_prefix("okutoon.com")
			.filter(|p| p.is_empty() || p.starts_with(['/', '?', '#']))
		else {
			return Ok(None);
		};
		let path = path
			.split(['?', '#'])
			.next()
			.unwrap_or_default()
			.trim_matches('/');
		let mut segments = path.split('/');
		let Some(series) = segments.next().filter(|s| !s.is_empty()) else {
			return Ok(None);
		};
		if NON_SERIES_PATHS.contains(&series) {
			return Ok(None);
		}
		Ok(Some(match segments.next().filter(|s| !s.is_empty()) {
			Some(chapter) => DeepLinkResult::Chapter {
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
