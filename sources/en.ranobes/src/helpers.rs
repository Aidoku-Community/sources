use aidoku::{
	Chapter, ContentRating, HomeComponent, HomeComponentValue, Link, LinkValue, Listing, Manga,
	MangaStatus, Result,
	alloc::{String, Vec, string::ToString, vec},
	imports::{html::Document, net::Request, std::parse_date},
	prelude::*,
};

use crate::BASE_URL;
use crate::models::{ChapterEntry, ChapterListData};
use crate::settings;

/// Extracts `(slug, novel_id)` from a chapter path of the form
/// `/{slug}-{novel_id}/{chapter_id}.html`. Confirmed against two different
/// novels: the same slug used in `/novels/{id}-{slug}.html` reappears here
/// with the slug and id swapped.
///
/// Returns `None` for anything that doesn't match this exact shape, so it's
/// safe to try on arbitrary paths (e.g. `/chapters/...`, `/search/...`)
/// without false positives.
pub fn parse_chapter_path(path: &str) -> Option<(String, String)> {
	let path = path.trim_start_matches('/');
	let (folder, file) = path.split_once('/')?;

	let chapter_id = file.strip_suffix(".html")?;
	if chapter_id.is_empty() || !chapter_id.chars().all(|c| c.is_ascii_digit()) {
		return None;
	}

	let dash_pos = folder.rfind('-')?;
	let (slug, id_part) = folder.split_at(dash_pos);
	let novel_id = &id_part[1..]; // drop the leading '-'
	if novel_id.is_empty() || slug.is_empty() || !novel_id.chars().all(|c| c.is_ascii_digit()) {
		return None;
	}

	Some((slug.to_string(), novel_id.to_string()))
}

pub fn request_html(url: &str) -> Result<Document> {
	Ok(Request::get(url)?.html()?)
}

/// Extracts the numeric novel id from a manga key of the form
/// `/novels/{id}-{slug}.html` (the id is the leading digit run of the
/// final path segment). Confirmed against a real fetched page.
pub fn novel_id_from_key(key: &str) -> Option<String> {
	let file = key.rsplit('/').next()?;
	let digits: String = file.chars().take_while(|c| c.is_ascii_digit()).collect();
	(!digits.is_empty()).then_some(digits)
}

pub fn parse_chapter_number(title: &str) -> Option<f32> {
	let after = title.strip_prefix("Chapter ")?;
	let digits: String = after
		.chars()
		.take_while(|c| c.is_ascii_digit() || *c == '.')
		.collect();
	digits.parse().ok()
}

/// Confirmed against real HTML in two places (search results and the
/// details page poster): the cover is always a CSS
/// `background-image: url(...)` on a `<figure class="cover">`, never an
/// `<img src>`/`data-src`. The details page does have an `<img>` near the
/// poster too, but it's an invisible click-target for a lightbox zoom
/// (`opacity: 0`), not the real cover — don't be tempted to read it.
fn find_cover_image(container: &aidoku::imports::html::Element) -> Option<String> {
	let figure = container.select_first("figure.cover")?;
	let style = figure.attr("style")?;
	let start = style.find("url(")? + 4;
	let end = start + style[start..].find(')')?;
	let url = style[start..end].trim_matches(|c: char| c == '\'' || c == '"');
	(!url.is_empty()).then(|| url.to_string())
}

pub fn parse_search_results(html: &Document) -> Vec<Manga> {
	let Some(containers) = html.select(".short-cont") else {
		return Vec::new();
	};
	containers
		.filter_map(|container| {
			let link = container.select_first(".title a")?;
			let url = link.attr("abs:href")?;
			let key = url.strip_prefix(BASE_URL)?.to_string();
			let title = link.text()?;
			let cover = find_cover_image(&container);
			Some(Manga {
				key,
				title,
				cover,
				url: Some(url),
				..Default::default()
			})
		})
		.collect()
}

/// Confirmed: the status is a link to `/tags/status-end/{value}/` — the
/// same `status-end` filter parameter already confirmed for search.
fn parse_status(html: &Document) -> MangaStatus {
	let value = html
		.select_first("a[href*='/tags/status-end/']")
		.and_then(|el| el.text());
	match value.as_deref() {
		Some("Ongoing") => MangaStatus::Ongoing,
		Some("Completed") => MangaStatus::Completed,
		Some("Hiatus") => MangaStatus::Hiatus,
		Some("Dropped") => MangaStatus::Cancelled,
		_ => MangaStatus::Unknown,
	}
}

pub fn content_rating_from_tags(tags: &[String]) -> ContentRating {
	const NSFW_TAGS: &[&str] = &["Adult", "Smut", "Mature"];
	const SUGGESTIVE_TAGS: &[&str] = &["Ecchi", "Harem", "Yaoi", "Yuri"];
	if tags.iter().any(|t| NSFW_TAGS.contains(&t.as_str())) {
		ContentRating::NSFW
	} else if tags.iter().any(|t| SUGGESTIVE_TAGS.contains(&t.as_str())) {
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	}
}

pub fn fill_manga_details(html: &Document, mut manga: Manga) -> Result<Manga> {
	let title = html
		.select_first("h1.title")
		.and_then(|el| el.text())
		.ok_or_else(|| error!("Title not found"))?;
	manga.title = title;

	manga.cover = html
		.select_first(".r-fullstory-poster .poster")
		.and_then(|el| find_cover_image(&el));

	manga.authors = html.select(".tag_list a[href*='/authors/']").map(|els| {
		els.filter_map(|el| el.text())
			.map(|t| t.trim().to_string())
			.collect::<Vec<_>>()
	});

	// Confirmed: genres live in a separate collapsible panel, not in
	// .tag_list. The panel is collapsed by default but the links are still
	// present in the markup.
	manga.tags = html.select("#mc-fs-genre .links a").map(|els| {
		els.filter_map(|el| el.text())
			.map(|t| t.trim().to_string())
			.collect::<Vec<_>>()
	});
	manga.content_rating = manga
		.tags
		.as_deref()
		.map(content_rating_from_tags)
		.unwrap_or_default();

	manga.description = html
		.select_first("div.cont-text.showcont-h")
		.and_then(|el| el.text())
		.map(|t| t.trim().to_string())
		.filter(|t| !t.is_empty());

	manga.status = parse_status(html);

	Ok(manga)
}

/// The `window.__DATA__` blob can be on any `<script>` tag on the page (no
/// specific container class is confirmed), so every script tag's contents
/// are scanned for the prefix, same approach en.batcave uses for its own
/// page-image script (as opposed to its chapter-list script, which does have
/// a known container — ranobes' container is not confirmed, so the broader
/// scan is the safer choice here).
fn extract_data_blob(html: &Document) -> Result<ChapterListData> {
	let scripts = html
		.select("script")
		.ok_or_else(|| error!("No script tags found"))?;
	let raw = scripts
		.filter_map(|el| el.data())
		.find(|text| text.trim_start().starts_with("window.__DATA__"))
		.ok_or_else(|| error!("window.__DATA__ script not found"))?;
	let trimmed = raw.trim();
	let json_str = trimmed
		.strip_prefix("window.__DATA__ = ")
		.unwrap_or(trimmed);
	let json_str = json_str.strip_suffix(';').unwrap_or(json_str);
	serde_json::from_str(json_str).map_err(|_| error!("Failed to parse chapter list JSON"))
}

/// How many pages to request concurrently per batch. Sending all ~124
/// remaining pages for a novel like Shadow Slave in one single batch
/// turned out to be too aggressive — most requests were silently dropped
/// (likely rate-limited/blocked), returning only a handful of pages
/// instead of the full list. Batching keeps most of the speed benefit
/// over one-at-a-time while being much gentler on the site. This number
/// is a starting guess, not confirmed as optimal — may need tuning based
/// on further testing.
const CHAPTER_PAGE_BATCH_SIZE: usize = 10;

/// How many extra attempts a still-failing page within a batch gets
/// before being given up on. Real testing showed the number of pages
/// that fail varies between runs (5, 10, 9 failures across three
/// attempts for the same novel) rather than being consistent, which
/// points to transient/flaky failures rather than a hard, deterministic
/// block — so retrying is worth it, not just batching.
const CHAPTER_PAGE_MAX_RETRIES: u32 = 3;

/// Fetches every page of `/chapters/{id}/`, concatenating the `chapters`
/// arrays in page order (page 1 first, then page 2, etc.).
///
/// Confirmed newest-first (page 1 starts with the most recent chapter).
///
/// Page 1 is fetched alone first since it's the only way to learn
/// `pages_count`. Remaining pages are sent in small concurrent batches
/// (`Request::send_all`) rather than one at a time — for a long-running
/// novel like Shadow Slave (125 pages), 125 *sequential* requests risked
/// exceeding a reasonable load timeout, which matched a real report of
/// chapters silently coming back empty for novels with many pages while
/// short novels worked fine. Sending everything as a single giant batch
/// instead of sequential was tried first but was too aggressive (see
/// CHAPTER_PAGE_BATCH_SIZE). Pages that still fail within a batch are
/// retried a few times (see CHAPTER_PAGE_MAX_RETRIES) rather than
/// dropped immediately, since failures observed in testing were
/// inconsistent between attempts on the same novel.
///
/// Each page's chapters are written into an indexed slot (rather than
/// appended as responses/retries happen to complete) so the final order
/// always matches page order, even when a page only succeeds on a later
/// retry after later pages have already come back.
pub fn fetch_chapter_list(novel_key: &str) -> Result<Vec<Chapter>> {
	let novel_id = novel_id_from_key(novel_key)
		.ok_or_else(|| error!("Could not find novel id in {novel_key}"))?;

	let first_url = format!("{BASE_URL}/chapters/{novel_id}/");
	let first_html = request_html(&first_url)?;
	let first_data = extract_data_blob(&first_html)?;
	let pages_count = first_data.pages_count.max(1);

	let mut pages: Vec<Option<Vec<ChapterEntry>>> = vec![None; pages_count as usize];
	pages[0] = Some(first_data.chapters);

	let page_numbers: Vec<i32> = (2..=pages_count).collect();

	for chunk in page_numbers.chunks(CHAPTER_PAGE_BATCH_SIZE) {
		let mut pending: Vec<i32> = chunk.to_vec();
		for _ in 0..=CHAPTER_PAGE_MAX_RETRIES {
			if pending.is_empty() {
				break;
			}

			let mut sent_pages = Vec::with_capacity(pending.len());
			let mut reqs = Vec::with_capacity(pending.len());
			let mut still_pending = Vec::new();
			for &page in &pending {
				let url = format!("{BASE_URL}/chapters/{novel_id}/page/{page}/");
				match Request::get(&url) {
					Ok(req) => {
						sent_pages.push(page);
						reqs.push(req);
					}
					Err(_) => still_pending.push(page),
				}
			}

			for (page, response) in sent_pages.into_iter().zip(Request::send_all(reqs)) {
				match response
					.ok()
					.and_then(|r| r.get_html().ok())
					.and_then(|html| extract_data_blob(&html).ok())
				{
					Some(data) => pages[(page - 1) as usize] = Some(data.chapters),
					None => still_pending.push(page),
				}
			}

			pending = still_pending;
		}
		// Any pages still pending after all retries are silently dropped —
		// a partial chapter list is more useful than none at all.
	}

	let entries = pages.into_iter().flatten().flatten();

	Ok(entries
		.map(|entry| {
			let chapter_number = parse_chapter_number(&entry.title);
			// entry.link is a full absolute url (confirmed against real
			// data), so it's stored relative here to match how manga.key
			// works elsewhere (get_page_list re-prefixes with BASE_URL).
			let key = entry
				.link
				.strip_prefix(BASE_URL)
				.unwrap_or(&entry.link)
				.to_string();
			// Confirmed format: "2026-07-21 17:58:28".
			let date_uploaded = parse_date(&entry.date, "yyyy-MM-dd HH:mm:ss");
			Chapter {
				key,
				title: Some(entry.title),
				chapter_number,
				date_uploaded,
				url: Some(entry.link),
				..Default::default()
			}
		})
		.collect())
}

pub fn extract_chapter_text(html: &Document) -> Result<String> {
	let container = html
		.select_first("div#arrticle")
		.ok_or_else(|| error!("Chapter content not found"))?;
	let text = container
		.select("p")
		.map(|els| {
			els.filter_map(|el| el.text())
				.map(|t| t.trim().to_string())
				.filter(|t| !t.is_empty())
				.collect::<Vec<_>>()
				.join("\n\n")
		})
		.unwrap_or_default();
	if text.is_empty() {
		bail!("Chapter text not found");
	}
	Ok(text)
}

/// The site encodes spaces in path-segment values as `+` (confirmed via
/// `v.genre=Adult,Martial+Arts`). Other characters that would otherwise
/// break the `/f/...` path structure (or be ambiguous with the `+`-for-
/// space convention) are percent-escaped.
pub fn encode_value(value: &str) -> String {
	value
		.chars()
		.map(|c| match c {
			' ' => "+".to_string(),
			'/' => "%2F".to_string(),
			'?' => "%3F".to_string(),
			'#' => "%23".to_string(),
			'+' => "%2B".to_string(),
			'%' => "%25".to_string(),
			_ => c.to_string(),
		})
		.collect()
}

pub fn encode_list(values: &[String]) -> String {
	values
		.iter()
		.map(|v| encode_value(v))
		.collect::<Vec<_>>()
		.join(",")
}

pub const LISTING_LATEST: &str = "latest";
pub const LISTING_POPULAR: &str = "popular";
pub const LISTING_COMPLETED: &str = "completed";

/// Builds a `/f/` url for a Home/listing section, re-using the same
/// confirmed filter endpoint as search rather than needing new selectors.
/// Hidden genres/languages are applied here too, matching the person's
/// content-filter settings on the home page, not just in search.
pub fn build_listing_url(id: &str, page: i32) -> Option<String> {
	let mut segments: Vec<String> = Vec::new();

	let genres_ex = settings::hidden_genres();
	if !genres_ex.is_empty() {
		segments.push(format!("v.genre={}", encode_list(&genres_ex)));
	}
	let langs_ex = settings::hidden_languages();
	if !langs_ex.is_empty() {
		segments.push(format!("v.languages={}", encode_list(&langs_ex)));
	}

	match id {
		LISTING_LATEST => {
			segments.push("sort=date".to_string());
			segments.push("order=desc".to_string());
		}
		LISTING_POPULAR => {
			segments.push("sort=news_read".to_string());
			segments.push("order=desc".to_string());
		}
		LISTING_COMPLETED => {
			segments.push("status-end=Completed".to_string());
			segments.push("sort=date".to_string());
			segments.push("order=desc".to_string());
		}
		_ => return None,
	}

	if page > 1 {
		segments.push(format!("page/{page}"));
	}

	Some(format!("{BASE_URL}/f/{}/", segments.join("/")))
}

pub fn push_scroller(components: &mut Vec<HomeComponent>, title: &str, listing_id: &str) {
	let Some(url) = build_listing_url(listing_id, 1) else {
		return;
	};
	let Ok(html) = request_html(&url) else {
		return;
	};
	let entries = parse_search_results(&html);
	if entries.is_empty() {
		return;
	}
	let entries: Vec<Link> = entries
		.into_iter()
		.map(|manga| Link {
			title: manga.title.clone(),
			subtitle: None,
			image_url: manga.cover.clone(),
			value: Some(LinkValue::Manga(manga)),
		})
		.collect();
	components.push(HomeComponent {
		title: Some(title.into()),
		subtitle: None,
		value: HomeComponentValue::Scroller {
			entries,
			listing: Some(Listing {
				id: listing_id.into(),
				name: title.into(),
				..Default::default()
			}),
		},
	});
}
