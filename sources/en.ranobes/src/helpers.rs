use aidoku::{
	Chapter, ContentRating, HomeComponent, HomeComponentValue, Link, LinkValue, Listing, Manga,
	MangaStatus, Result,
	alloc::{String, Vec, string::ToString},
	imports::{html::Document, net::{Request, Response}, std::parse_date},
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

const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 \
	(KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1";

fn check_for_cf_challenge(response: &Response) -> Result<()> {
	if response.status_code() == 403
		&& response
			.get_header("cf-mitigated")
			.is_some_and(|value| value == "challenge")
	{
		bail!("Blocked by the site's bot protection (Cloudflare challenge)");
	}
	Ok(())
}

pub fn request_html(url: &str) -> Result<Document> {
	let response = Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.header("Referer", BASE_URL)
		.send()?;
	check_for_cf_challenge(&response)?;
	Ok(response.get_html()?)
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

/// How many pages to request concurrently per batch. Fetching all
/// remaining pages for a long novel in one batch is too aggressive — the
/// site starts dropping/blocking requests, returning only a handful of
/// pages instead of the full list. Batching keeps most of the speed
/// benefit of concurrent requests while staying gentler on the site.
const CHAPTER_PAGE_BATCH_SIZE: usize = 5;

/// How many extra attempts a still-failing page within a batch gets
/// before being given up on. The number of pages that fail varies
/// between runs for the same novel rather than being consistent,
/// pointing to transient failures rather than a hard block — so
/// retrying pages within a batch is worth it, not just batching itself.
const CHAPTER_PAGE_MAX_RETRIES: u32 = 5;

/// Fetches every page of `/chapters/{id}/` and returns chapters in
/// newest-to-oldest order.
///
/// Page 1 is fetched alone first since it's the only way to learn
/// `pages_count`. Remaining pages are then requested **oldest-first**
/// (highest page number first) in small concurrent batches
/// (`Request::send_all`), with a few retries per batch — for a
/// long-running novel like Shadow Slave (125 pages), sequential
/// one-at-a-time requests risked exceeding a reasonable load timeout, and
/// sending everything as one giant batch was too aggressive (see
/// CHAPTER_PAGE_BATCH_SIZE).
///
/// The oldest-first order matters for graceful degradation: if the site
/// starts blocking partway through (observed in testing — pagination and
/// even other novels' chapter lists stopped working mid-session after
/// heavy use), the pages fetched *before* the block hit are kept, and
/// everything after the first gap is discarded rather than returned with
/// a hole in the middle. Page 1's newest chapters are only included if
/// every other page also succeeded — an isolated newest chapter far
/// removed from an otherwise-incomplete list would be a more confusing
/// reading experience than simply not showing it yet.
/// Walks backward from the last page (oldest chapters) and keeps the
/// contiguous run of successfully fetched pages, stopping at the first
/// gap. Page 1's newest chapters are only included if every other page
/// also succeeded (`pages.len() - 1` entries recovered) — see
/// `fetch_chapter_list`'s doc comment for why a partial run doesn't
/// include them.
fn contiguous_chapters_from_end(pages: &mut [Option<Vec<ChapterEntry>>]) -> Vec<ChapterEntry> {
	let pages_count = pages.len();
	let mut contiguous_from_end = Vec::new();
	for page_index in (1..pages_count).rev() {
		if pages[page_index].is_none() {
			break;
		}
		contiguous_from_end.push(page_index);
	}
	contiguous_from_end.reverse(); // ascending page order (oldest run, newest-of-the-run first)

	let complete = contiguous_from_end.len() == pages_count - 1;

	let mut ordered_chapters = Vec::new();
	if complete {
		ordered_chapters.extend(pages[0].take().unwrap_or_default());
	}
	for page_index in contiguous_from_end {
		ordered_chapters.extend(pages[page_index].take().unwrap_or_default());
	}
	ordered_chapters
}

pub fn fetch_chapter_list(novel_key: &str) -> Result<Vec<Chapter>> {
	let novel_id = novel_id_from_key(novel_key)
		.ok_or_else(|| error!("Could not find novel id in {novel_key}"))?;

	let first_url = format!("{BASE_URL}/chapters/{novel_id}/");
	let first_html = request_html(&first_url)?;
	let first_data = extract_data_blob(&first_html)?;
	let pages_count = first_data.pages_count.max(1);

	let mut pages: Vec<Option<Vec<ChapterEntry>>> = (0..pages_count).map(|_| None).collect();
	pages[0] = Some(first_data.chapters);

	// Oldest page (highest number) first.
	let page_numbers: Vec<i32> = (2..=pages_count).rev().collect();

	let mut blocked = false;
	for chunk in page_numbers.chunks(CHAPTER_PAGE_BATCH_SIZE) {
		if blocked {
			break;
		}
		let mut pending: Vec<i32> = chunk.to_vec();
		for _ in 0..=CHAPTER_PAGE_MAX_RETRIES {
			if pending.is_empty() || blocked {
				break;
			}

			let mut sent_pages = Vec::with_capacity(pending.len());
			let mut reqs = Vec::with_capacity(pending.len());
			let mut still_pending = Vec::new();
			for &page in &pending {
				let url = format!("{BASE_URL}/chapters/{novel_id}/page/{page}/");
				match Request::get(&url).map(|req| {
					req.header("User-Agent", USER_AGENT)
						.header("Referer", BASE_URL)
				}) {
					Ok(req) => {
						sent_pages.push(page);
						reqs.push(req);
					}
					Err(_) => still_pending.push(page),
				}
			}

			for (page, response) in sent_pages.into_iter().zip(Request::send_all(reqs)) {
				let Ok(response) = response else {
					still_pending.push(page);
					continue;
				};
				if check_for_cf_challenge(&response).is_err() {
					// A challenge won't clear by retrying, and further
					// pages are likely to hit it too — stop immediately
					// rather than burning through retries and remaining
					// batches on requests that won't succeed. Whatever
					// pages already succeeded are kept via the contiguous-
					// run logic below, same as any other partial failure.
					blocked = true;
					still_pending.push(page);
					continue;
				}
				match response
					.get_html()
					.ok()
					.and_then(|html| extract_data_blob(&html).ok())
				{
					Some(data) => pages[(page - 1) as usize] = Some(data.chapters),
					None => still_pending.push(page),
				}
			}

			pending = still_pending;
		}
		// Any pages still pending after all retries stop the whole batch
		// loop early below rather than being silently skipped over, so a
		// later successful page can't create a gap.
		if !pending.is_empty() {
			break;
		}
	}

	let ordered_chapters = contiguous_chapters_from_end(&mut pages);

	Ok(ordered_chapters
		.into_iter()
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

#[cfg(test)]
mod tests {
	use super::*;

	fn entry(title: &str) -> ChapterEntry {
		ChapterEntry {
			title: title.to_string(),
			link: String::new(),
			date: String::new(),
		}
	}

	#[test]
	fn contiguous_chapters_from_end_returns_everything_when_complete() {
		let mut pages = vec![
			Some(vec![entry("Chapter 3")]),
			Some(vec![entry("Chapter 2")]),
			Some(vec![entry("Chapter 1")]),
		];
		let chapters = contiguous_chapters_from_end(&mut pages);
		let titles: Vec<&str> = chapters.iter().map(|c| c.title.as_str()).collect();
		assert_eq!(titles, ["Chapter 3", "Chapter 2", "Chapter 1"]);
	}

	#[test]
	fn contiguous_chapters_from_end_drops_newest_page_on_gap() {
		// page 1 (newest) succeeded, middle page failed, oldest page succeeded
		let mut pages = vec![Some(vec![entry("Chapter 3")]), None, Some(vec![entry("Chapter 1")])];
		let chapters = contiguous_chapters_from_end(&mut pages);
		let titles: Vec<&str> = chapters.iter().map(|c| c.title.as_str()).collect();
		assert_eq!(titles, ["Chapter 1"]);
	}

	#[test]
	fn contiguous_chapters_from_end_keeps_partial_run_from_the_end() {
		let mut pages = vec![
			None, // page 1 (newest) failed
			Some(vec![entry("Chapter 2")]),
			Some(vec![entry("Chapter 1")]),
		];
		let chapters = contiguous_chapters_from_end(&mut pages);
		let titles: Vec<&str> = chapters.iter().map(|c| c.title.as_str()).collect();
		assert_eq!(titles, ["Chapter 2", "Chapter 1"]);
	}
}
