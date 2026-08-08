#![no_std]

use aidoku::{
	AidokuError, Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Home,
	HomeLayout, Listing, ListingProvider, Manga, MangaPageResult, MangaStatus, Page, PageContent,
	Result, Source, Viewer,
	alloc::{String, Vec, format, vec},
	imports::{js::JsContext, net::Request},
	prelude::*,
};
use serde::Deserialize;

// ─── Constants ────────────────────────────────────────────────────────────────

const BASE_URL: &str = "https://theblank.net";

// ─── JSON types ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct LibrarySerie {
	title: String,
	image: String,
	link: String,
	#[serde(rename = "serie_status")]
	status: String,
	#[serde(rename = "genres_slugs")]
	genres: Vec<String>,
}

#[derive(Deserialize)]
struct LibraryMeta {
	current_page: i32,
	last_page: i32,
}

#[derive(Deserialize)]
struct LibrarySeriesWrapper {
	data: Vec<LibrarySerie>,
	meta: LibraryMeta,
}

#[derive(Deserialize)]
struct LibraryProps {
	series: LibrarySeriesWrapper,
}

#[derive(Deserialize)]
struct InertiaPage<T> {
	props: T,
}

#[derive(Deserialize)]
struct SerieChapter {
	title: String,
	slug: String,
	#[serde(rename = "chapterNumber")]
	chapter_number: f32,
	#[serde(rename = "createdAt")]
	created_at: String,
	thumbnail: Option<String>,
}

#[derive(Deserialize)]
struct SerieGenre {
	name: String,
}

#[derive(Deserialize)]
struct SerieDetail {
	name: String,
	slug: String,
	description: String,
	author: String,
	cover_image: String,
	status: String,
	genres: Vec<SerieGenre>,
	chapters: Vec<SerieChapter>,
}

#[derive(Deserialize)]
struct SerieDetailProps {
	serie: SerieDetail,
}

#[derive(Deserialize)]
struct ChapterSerie {
	slug: String,
}

#[derive(Deserialize)]
struct ChapterData {
	slug: String,
	page_count: i32,
	chapter_token: String,
	serie: ChapterSerie,
}

#[derive(Deserialize)]
struct ChapterReaderProps {
	data: ChapterData,
}

#[derive(Deserialize)]
struct HmacParams {
	ts: String,
	nonce: String,
	sig: String,
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Extract and parse the Inertia `data-page` JSON from the HTML.
fn parse_inertia<T: serde::de::DeserializeOwned>(html: &str) -> Option<T> {
	let marker = "data-page=\"";
	let start = html.find(marker)? + marker.len();
	let rest = &html[start..];
	let end = rest.find("\">")?;
	let encoded = &rest[..end];

	let decoded = encoded
		.replace("&quot;", "\"")
		.replace("&amp;", "&")
		.replace("&#039;", "'")
		.replace("&lt;", "<")
		.replace("&gt;", ">");

	serde_json::from_str::<InertiaPage<T>>(&decoded)
		.ok()
		.map(|p| p.props)
}

fn manga_status(s: &str) -> MangaStatus {
	match s {
		"ongoing" => MangaStatus::Ongoing,
		"finished" => MangaStatus::Completed,
		"dropped" => MangaStatus::Cancelled,
		"onhold" => MangaStatus::Hiatus,
		_ => MangaStatus::Unknown,
	}
}

/// Parse ISO-8601 datetime → Unix timestamp in seconds.
fn parse_iso_date(s: &str) -> i64 {
	let b = s.as_bytes();
	if b.len() < 19 {
		return 0;
	}
	let year = digits(&b[0..4]) as i64;
	let month = digits(&b[5..7]) as i64;
	let day = digits(&b[8..10]) as i64;
	let hour = digits(&b[11..13]) as i64;
	let min = digits(&b[14..16]) as i64;
	let sec = digits(&b[17..19]) as i64;

	let days_per_month: [i64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
	let leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);

	let y = year - 1;
	let total_days = y * 365 + y / 4 - y / 100 + y / 400;
	let epoch_y = 1969i64;
	let epoch_days = epoch_y * 365 + epoch_y / 4 - epoch_y / 100 + epoch_y / 400;
	let mut days = total_days - epoch_days;
	for (m, &month_days) in days_per_month.iter().enumerate().take((month - 1) as usize) {
		days += month_days;
		if m == 1 && leap {
			days += 1;
		}
	}
	days += day - 1;

	days * 86400 + hour * 3600 + min * 60 + sec
}

fn digits(b: &[u8]) -> u32 {
	b.iter().fold(0u32, |a, &c| a * 10 + (c - b'0') as u32)
}

fn abs_url(path: &str) -> String {
	if path.starts_with('/') {
		format!("{}{}", BASE_URL, path)
	} else {
		String::from(path)
	}
}

/// Build a signed page image URL via HMAC-SHA256 computed inside JsContext.
///
/// Signing observed from pam.wasm:
///   message = hex(page_byte) + decimal_ts_string + hex_nonce (16 chars)
///   key     = chapter_token hex-decoded
///   sig     = HMAC-SHA256(key, message) hex
fn build_page_url(serie_slug: &str, chapter_slug: &str, token: &str, page: i32) -> String {
	let js = format!(
		r#"(async () => {{
            const token = '{token}';
            const page = {page};
            const ts = Math.floor(Date.now() / 1000);
            const nonceArr = new Uint8Array(8);
            crypto.getRandomValues(nonceArr);
            const nonce = Array.from(nonceArr).map(b => b.toString(16).padStart(2,'0')).join('');
            const keyBytes = new Uint8Array(token.match(/.{{2}}/g).map(b => parseInt(b,16)));
            const key = await crypto.subtle.importKey('raw', keyBytes, {{name:'HMAC',hash:'SHA-256'}}, false, ['sign']);
            const msg = page.toString(16).padStart(2,'0') + ts.toString() + nonce;
            const sig = Array.from(new Uint8Array(await crypto.subtle.sign('HMAC', key, new TextEncoder().encode(msg))))
                .map(b => b.toString(16).padStart(2,'0')).join('');
            return JSON.stringify({{ts: ts.toString(), nonce, sig}});
        }})()"#,
		token = token,
		page = page,
	);

	let result = JsContext::new().eval_async(&js).unwrap_or_default();
	let params: Option<HmacParams> = serde_json::from_str(&result).ok();

	match params {
		Some(p) => format!(
			"{}/serie/{}/chapter/{}/page/{}?token={}&ts={}&nonce={}&sig={}",
			BASE_URL, serie_slug, chapter_slug, page, token, p.ts, p.nonce, p.sig
		),
		None => format!(
			"{}/serie/{}/chapter/{}/page/{}?token={}",
			BASE_URL, serie_slug, chapter_slug, page, token
		),
	}
}

/// Fetch a URL with a mobile User-Agent to pass Cloudflare checks.
fn fetch_html(url: &str) -> Result<String> {
	Request::get(url)
		.map_err(|e| AidokuError::Message(format!("request error: {:?}", e)))?
		.header(
			"User-Agent",
			"Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) \
             AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1",
		)
		.header("Referer", BASE_URL)
		.string()
}

// ─── Source ───────────────────────────────────────────────────────────────────

struct Theblank;

impl Source for Theblank {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut url = format!("{}/library?page={}", BASE_URL, page);

		if let Some(q) = &query
			&& !q.is_empty()
		{
			url = format!("{}&search={}", url, q);
		}

		for filter in &filters {
			match filter {
				FilterValue::Select { id, value } => match id.as_str() {
					"orderby" => {
						url = format!("{}&orderby={}", url, value);
					}
					"status" if !value.is_empty() => {
						url = format!("{}&status[]={}", url, value);
					}
					_ => {}
				},
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} => {
					if id == "genres" {
						for g in included {
							url = format!("{}&include_genres[]={}", url, g);
						}
						for g in excluded {
							url = format!("{}&exclude_genres[]={}", url, g);
						}
					}
				}
				FilterValue::Text { id, value } if id == "search" && !value.is_empty() => {
					url = format!("{}&search={}", url, value);
				}
				_ => {}
			}
		}

		let html = fetch_html(&url)?;
		let props: LibraryProps = parse_inertia(&html).ok_or(AidokuError::Message(
			String::from("failed to parse library"),
		))?;

		let has_next = props.series.meta.current_page < props.series.meta.last_page;
		let entries = props
			.series
			.data
			.into_iter()
			.map(|s| Manga {
				key: s.link,
				title: s.title,
				cover: Some(abs_url(&s.image)),
				tags: Some(s.genres),
				status: manga_status(&s.status),
				content_rating: ContentRating::NSFW,
				viewer: Viewer::Webtoon,
				..Default::default()
			})
			.collect();

		Ok(MangaPageResult {
			entries,
			has_next_page: has_next,
		})
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

		let url = format!("{}{}", BASE_URL, manga.key);
		let html = fetch_html(&url)?;
		let props: SerieDetailProps = parse_inertia(&html).ok_or(AidokuError::Message(
			String::from("failed to parse serie page"),
		))?;

		let s = props.serie;
		let tags: Vec<String> = s.genres.into_iter().map(|g| g.name).collect();

		let chapters = if needs_chapters {
			Some(
				s.chapters
					.into_iter()
					.map(|c| {
						let key = format!("{}|{}", s.slug, c.slug);
						Chapter {
							key,
							title: Some(c.title),
							chapter_number: Some(c.chapter_number),
							date_uploaded: Some(parse_iso_date(&c.created_at)),
							thumbnail: c.thumbnail.map(|t| abs_url(&t)),
							language: Some(String::from("en")),
							url: Some(format!("{}/serie/{}/chapter/{}", BASE_URL, s.slug, c.slug)),
							..Default::default()
						}
					})
					.collect(),
			)
		} else {
			None
		};

		Ok(Manga {
			key: manga.key,
			title: s.name,
			cover: Some(abs_url(&s.cover_image)),
			description: Some(s.description),
			authors: Some(vec![s.author]),
			tags: Some(tags),
			status: manga_status(&s.status),
			content_rating: ContentRating::NSFW,
			viewer: Viewer::Webtoon,
			chapters,
			..Default::default()
		})
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		// chapter.key = "{serie_slug}|{chapter_slug}"
		let sep = chapter
			.key
			.find('|')
			.ok_or(AidokuError::Message(String::from("bad chapter key")))?;
		let serie_slug = &chapter.key[..sep];
		let chapter_slug = &chapter.key[sep + 1..];

		let url = format!("{}/serie/{}/chapter/{}", BASE_URL, serie_slug, chapter_slug);
		let html = fetch_html(&url)?;
		let props: ChapterReaderProps = parse_inertia(&html).ok_or(AidokuError::Message(
			String::from("failed to parse chapter page"),
		))?;

		let d = props.data;
		let sr_slug = d.serie.slug;
		let ch_slug = d.slug;
		let token = d.chapter_token;

		let pages = (1..=d.page_count)
			.map(|i| Page {
				content: PageContent::url(build_page_url(&sr_slug, &ch_slug, &token, i)),
				..Default::default()
			})
			.collect();

		Ok(pages)
	}
}

// ─── Listing provider ─────────────────────────────────────────────────────────

impl ListingProvider for Theblank {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let orderby = match listing.id.as_str() {
			"trending" => "trending",
			"recently" => "recently",
			"views" => "views",
			"alphabetical" => "alphabetical",
			_ => "date",
		};
		let url = format!("{}/library?page={}&orderby={}", BASE_URL, page, orderby);
		let html = fetch_html(&url)?;
		let props: LibraryProps = parse_inertia(&html).ok_or(AidokuError::Message(
			String::from("failed to parse library"),
		))?;

		let has_next = props.series.meta.current_page < props.series.meta.last_page;
		let entries = props
			.series
			.data
			.into_iter()
			.map(|s| Manga {
				key: s.link,
				title: s.title,
				cover: Some(abs_url(&s.image)),
				tags: Some(s.genres),
				status: manga_status(&s.status),
				content_rating: ContentRating::NSFW,
				viewer: Viewer::Webtoon,
				..Default::default()
			})
			.collect();

		Ok(MangaPageResult {
			entries,
			has_next_page: has_next,
		})
	}
}

// ─── Home ─────────────────────────────────────────────────────────────────────

impl Home for Theblank {
	fn get_home(&self) -> Result<HomeLayout> {
		Err(AidokuError::Unimplemented)
	}
}

// ─── Deep link ────────────────────────────────────────────────────────────────

impl DeepLinkHandler for Theblank {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url.strip_prefix(BASE_URL).unwrap_or(url.as_str());

		if let Some(rest) = path.strip_prefix("/serie/") {
			if let Some(ch_idx) = rest.find("/chapter/") {
				let serie_slug = &rest[..ch_idx];
				let chapter_slug = &rest[ch_idx + "/chapter/".len()..];
				return Ok(Some(DeepLinkResult::Chapter {
					manga_key: format!("/serie/{}", serie_slug),
					key: format!("{}|{}", serie_slug, chapter_slug),
				}));
			}
			return Ok(Some(DeepLinkResult::Manga {
				key: format!("/serie/{}", rest),
			}));
		}

		Ok(None)
	}
}

register_source!(Theblank, ListingProvider, Home, DeepLinkHandler);
