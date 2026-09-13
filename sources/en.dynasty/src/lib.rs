#![no_std]

mod models;

use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, HashMap, Home,
	HomeComponent, HomeComponentValue, HomeLayout, HomePartialResult, Link, Listing, ListingKind,
	ListingProvider, Manga, MangaPageResult, MangaStatus, Page, PageContent, Result, Source,
	Viewer,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::{
		defaults::{defaults_get_map, defaults_set_data},
		html::{Document, Element, Html},
		net::{Request, TimeUnit, set_rate_limit},
		std::{parse_date, send_partial_result},
	},
	prelude::*,
};
use models::*;

const BASE_URL: &str = "https://dynasty-scans.com";

struct Dynasty;

impl Source for Dynasty {
	fn new() -> Self {
		// Dynasty throttles aggressive clients; 1 request/second is the rate the
		// Mihon extension settled on after site-side 503 errors (keiyoushi PR #3218).
		set_rate_limit(1, 1, TimeUnit::Seconds);
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		// the site 500s on page 0; guard against any host quirk
		let page = page.max(1);
		// the default (no query, no filters) search state is plain browsing —
		// use the recently-added join so it gets covers, which the search
		// page's own listings don't have
		if query.as_ref().is_none_or(|q| q.is_empty()) && filters.is_empty() {
			return self.recently_added(page);
		}
		// normalize whitespace (IMEs can insert full-width spaces the site
		// search would treat literally)
		let query = query.map(|q| q.split_whitespace().collect::<Vec<_>>().join(" "));

		let mut qs = QueryParameters::new();
		if let Some(query) = query.as_ref()
			&& !query.is_empty()
		{
			qs.push_encoded("q", Some(&encode_query_value(query)));
		}

		let mut classes: Vec<&str> = Vec::new();
		for value in filters {
			match value {
				FilterValue::Sort { index, .. } => match index {
					1 => qs.push("sort", Some("name")),
					2 => qs.push("sort", Some("created_at")),
					3 => qs.push("sort", Some("released_on")),
					_ => {}
				},
				FilterValue::Check { id, value: 1 } => match id.as_str() {
					"type_series" => classes.push("Series"),
					"type_doujin" => classes.push("Doujin"),
					"type_anthology" => classes.push("Anthology"),
					_ => {}
				},
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} if id == "tags" => {
					for tag in included {
						qs.push_encoded("with[]", Some(&tag));
					}
					for tag in excluded {
						qs.push_encoded("without[]", Some(&tag));
					}
				}
				_ => {}
			}
		}
		// Without a class filter the search page only lists chapter-level
		// entries, so search across all item types by default.
		if classes.is_empty() {
			classes = vec!["Series", "Doujin", "Anthology"];
		}
		for class in &classes {
			qs.push_encoded("classes[]", Some(class));
		}
		qs.push("page", Some(&page.to_string()));

		let url = format!("{BASE_URL}/search?{qs}");
		let mut html = fetch_search_html(&url)?;

		let mut entries = parse_search_entries(&html);

		// The site search matches the full phrase only; when it yields nothing,
		// retry with trailing words dropped (e.g. a title fragment that doesn't
		// account for punctuation like "Class's").
		if entries.is_empty()
			&& let Some(query) = query.as_ref()
		{
			let mut words: Vec<&str> = query.split_whitespace().collect();
			let mut attempts = 0;
			while words.len() > 1 && attempts < 3 {
				words.pop();
				attempts += 1;
				let mut retry = QueryParameters::new();
				retry.push_encoded("q", Some(&encode_query_value(&words.join(" "))));
				for class in &classes {
					retry.push_encoded("classes[]", Some(class));
				}
				retry.push("page", Some(&page.to_string()));
				html = fetch_search_html(&format!("{BASE_URL}/search?{retry}"))?;
				entries = parse_search_entries(&html);
				if !entries.is_empty() {
					break;
				}
			}
		}

		let has_next_page = has_next_search_page(&html, page);

		// search pages carry no cover data; show cached real covers for
		// previously visited items (instant), nothing for the rest
		self.apply_real_covers(&mut entries, 0);

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let json: SiteItemJson =
			Request::get(format!("{BASE_URL}/{}.json", manga.key))?.json_owned()?;

		if needs_details {
			if !json.name.is_empty() {
				manga.title = json.name.clone();
			}
			manga.cover = json.cover.as_ref().map(|c| format!("{BASE_URL}{c}"));
			manga.url = Some(format!("{BASE_URL}/{}", manga.key));

			let mut authors = Vec::new();
			let mut artists = Vec::new();
			let mut tags = Vec::new();
			let mut status = MangaStatus::Unknown;
			// the site labels adult content with explicit general tags
			let mut nsfw = false;
			let mut ecchi = false;
			let mut read_ltr = false;
			let mut long_strip = false;
			for tag in &json.tags {
				match tag.tag_type.as_str() {
					"Author" => authors.push(tag.name.clone()),
					"Artist" => artists.push(tag.name.clone()),
					"Status" => {
						status = match tag.name.as_str() {
							"Ongoing" => MangaStatus::Ongoing,
							"Completed" => MangaStatus::Completed,
							"Hiatus" => MangaStatus::Hiatus,
							"Cancelled" => MangaStatus::Cancelled,
							_ => MangaStatus::Unknown,
						}
					}
					"General" => {
						match tag.name.as_str() {
							"NSFW" => nsfw = true,
							"Ecchi" => ecchi = true,
							"Read left to right" => read_ltr = true,
							"Long strip" => long_strip = true,
							_ => {}
						}
						tags.push(tag.name.clone());
					}
					_ => {}
				}
			}
			if !authors.is_empty() {
				manga.authors = Some(authors);
			}
			if !artists.is_empty() {
				manga.artists = Some(artists);
			}
			if !tags.is_empty() {
				manga.tags = Some(tags);
			}
			manga.status = status;
			manga.content_rating = if nsfw {
				ContentRating::NSFW
			} else if ecchi {
				ContentRating::Suggestive
			} else {
				ContentRating::Safe
			};
			manga.viewer = if read_ltr {
				Viewer::LeftToRight
			} else if long_strip {
				Viewer::Webtoon
			} else {
				Viewer::RightToLeft
			};

			let mut description = String::new();
			if let Some(aliases) = json.aliases.as_ref().and_then(aliases_list) {
				description.push_str(&format!("Aliases: {}\n\n", aliases.join(", ")));
			}
			if let Some(text) = json.description.as_deref().and_then(strip_html) {
				description.push_str(&text);
			}
			if !description.is_empty() {
				manga.description = Some(description);
			}
		}

		if needs_chapters {
			let mut chapters: Vec<Chapter> = Vec::new();
			let mut volume: Option<f32> = None;
			for tagging in &json.taggings {
				if let Some(header) = tagging.header.as_deref() {
					volume = header
						.split_once(' ')
						.and_then(|(_, n)| n.trim().parse::<f32>().ok());
					continue;
				}
				let Some(permalink) = tagging.permalink.as_deref() else {
					continue;
				};
				let scanlators: Vec<String> = tagging
					.tags
					.iter()
					.filter(|t| t.tag_type == "Scanlator")
					.map(|t| t.name.clone())
					.collect();
				chapters.push(Chapter {
					key: String::from(permalink),
					title: tagging.title.clone(),
					chapter_number: chapter_number_from_slug(permalink),
					volume_number: volume,
					date_uploaded: tagging
						.released_on
						.as_deref()
						.and_then(|d| parse_date(d, "yyyy-MM-dd")),
					scanlators: (!scanlators.is_empty()).then_some(scanlators),
					url: Some(format!("{BASE_URL}/chapters/{permalink}")),
					language: Some(String::from("en")),
					..Default::default()
				});
			}
			// taggings order varies per item; newest-first is deterministic
			chapters.sort_by(|a, b| {
				b.date_uploaded
					.unwrap_or(0)
					.cmp(&a.date_uploaded.unwrap_or(0))
			});
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let json: ChapterJson =
			Request::get(format!("{BASE_URL}/chapters/{}.json", chapter.key))?.json_owned()?;
		Ok(json
			.pages
			.iter()
			.map(|p| Page {
				content: PageContent::url(format!("{BASE_URL}{}", p.url)),
				..Default::default()
			})
			.collect())
	}
}

impl Dynasty {
	/// GET /chapters/added.json joined with the /chapters page, whose rows
	/// carry a release thumbnail per chapter permalink (the only cover data
	/// available outside item pages).
	fn joined_recent_entries(&self, page: i32) -> Result<MangaPageResult> {
		let page = page.max(1);
		let json: AddedResponse =
			Request::get(format!("{BASE_URL}/chapters/added.json?page={page}"))?.json_owned()?;
		let thumbnails = Self::release_thumbnails(
			&Request::get(format!("{BASE_URL}/chapters?page={page}"))?.html()?,
		);

		let mut entries: Vec<Manga> = Vec::new();
		let mut seen: Vec<String> = Vec::new();
		for chapter in &json.chapters {
			let Some((namespace, name, permalink)) = classify(&chapter.tags) else {
				continue;
			};
			let key = format!("{namespace}/{permalink}");
			if seen.contains(&key) {
				continue;
			}
			seen.push(key.clone());
			entries.push(Manga {
				key,
				title: name,
				cover: thumbnails.get(&chapter.permalink).cloned(),
				..Default::default()
			});
		}

		Ok(MangaPageResult {
			entries,
			has_next_page: page < json.total_pages,
		})
	}

	/// The "Recently Added" listing: joined entries; listings stay fast, so
	/// only already-cached real covers are applied (no fresh fetches).
	fn recently_added(&self, page: i32) -> Result<MangaPageResult> {
		let mut result = self.joined_recent_entries(page)?;
		self.apply_real_covers(&mut result.entries, 0);
		Ok(result)
	}

	/// Chapter permalink → release thumbnail, from a /chapters listing page.
	fn release_thumbnails(html: &Document) -> HashMap<String, String> {
		let mut map = HashMap::new();
		if let Some(rows) = html.select("a.chapter") {
			for row in rows {
				let Some(href) = row.attr("href") else {
					continue;
				};
				let Some(permalink) = href.strip_prefix("/chapters/") else {
					continue;
				};
				let Some(src) = row
					.select_first("img")
					.and_then(|img| img.attr("src"))
					.filter(|src| src.starts_with('/'))
				else {
					continue;
				};
				map.insert(String::from(permalink), format!("{BASE_URL}{src}"));
			}
		}
		map
	}

	/// Swap thumbnail covers for the items' real covers, which only exist in
	/// each item's JSON. Cached covers apply instantly; at most `budget`
	/// uncached items are fetched per call since the site only tolerates ~1
	/// request/second. The cache holds one URL string per item (capped), so
	/// fetched covers are retained until the bounded cache reaches capacity.
	fn apply_real_covers(&self, entries: &mut [Manga], budget: i32) {
		const COVER_CACHE_KEY: &str = "coverCache";
		const COVER_CACHE_CAP: usize = 2000;
		let mut cache = defaults_get_map(COVER_CACHE_KEY).unwrap_or_default();
		let mut fetched = 0;
		let mut updated = false;
		for manga in entries.iter_mut() {
			let key = manga.key.clone();
			if let Some(cover) = cache.get(&key) {
				manga.cover = Some(cover.clone());
				continue;
			}
			if fetched >= budget || cache.len() >= COVER_CACHE_CAP {
				continue;
			}
			fetched += 1;
			let real = Request::get(format!("{BASE_URL}/{key}.json"))
				.ok()
				.and_then(|r| r.json_owned::<SiteItemJson>().ok())
				.and_then(|item| item.cover.map(|c| format!("{BASE_URL}{c}")));
			match real {
				Some(cover) => {
					println!("cover ok: {key}");
					cache.insert(key, cover.clone());
					manga.cover = Some(cover);
					updated = true;
				}
				None => println!("cover miss: {key}"),
			}
		}
		if updated {
			defaults_set_data(COVER_CACHE_KEY, &cache);
		}
	}

	fn home_layout(entries: Vec<Manga>) -> HomeLayout {
		let mut components = Vec::new();
		if entries.len() > 3 {
			components.push(HomeComponent {
				title: Some(String::from("Recently Added")),
				subtitle: None,
				value: HomeComponentValue::BigScroller {
					entries: entries[..3].to_vec(),
					auto_scroll_interval: Some(10.0),
				},
			});
		}
		components.push(HomeComponent {
			title: Some(String::from("Latest Releases")),
			subtitle: None,
			value: HomeComponentValue::MangaList {
				ranking: false,
				page_size: Some(6),
				entries: entries.into_iter().map(Link::from).collect(),
				listing: Some(Listing {
					id: String::from("added"),
					name: String::from("Recently Added"),
					kind: ListingKind::Default,
				}),
			},
		});
		HomeLayout { components }
	}
}

impl Home for Dynasty {
	fn get_home(&self) -> Result<HomeLayout> {
		let mut result = self.joined_recent_entries(1)?;
		// first paint: thumbnails (plus any cached real covers)
		self.apply_real_covers(&mut result.entries, 0);
		send_partial_result(&HomePartialResult::Layout(Self::home_layout(
			result.entries.clone(),
		)));
		// then upgrade up to 8 items with real covers and replace the layout
		self.apply_real_covers(&mut result.entries, 8);
		Ok(Self::home_layout(result.entries))
	}
}

impl ListingProvider for Dynasty {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		match listing.id.as_str() {
			"added" => self.recently_added(page),
			_ => bail!("Unknown listing: {}", listing.id),
		}
	}
}

impl DeepLinkHandler for Dynasty {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};
		let path = path.trim_start_matches('/');

		if path.starts_with("series/")
			|| path.starts_with("doujins/")
			|| path.starts_with("anthologies/")
		{
			return Ok(Some(DeepLinkResult::Manga {
				key: String::from(path),
			}));
		}

		if let Some(slug) = path.strip_prefix("chapters/") {
			// chapter links don't carry their parent key; resolve it via the JSON
			let json: ChapterJson =
				Request::get(format!("{BASE_URL}/chapters/{slug}.json"))?.json_owned()?;
			if let Some((namespace, _, permalink)) = classify(&json.tags) {
				return Ok(Some(DeepLinkResult::Chapter {
					manga_key: format!("{namespace}/{permalink}"),
					key: String::from(slug),
				}));
			}
		}

		Ok(None)
	}
}

/// Form-style encoding for search query values: spaces become "+" (which the
/// server decodes back to a space) and URL-structural characters are
/// percent-encoded. Unlike a full percent-encode this keeps the value free of
/// "%XX" sequences for the common case — some network middlemen mangle those
/// (e.g. %20 → literal or double-encoded), which made multi-word searches
/// return empty results.
fn encode_query_value(value: &str) -> String {
	let mut out = String::with_capacity(value.len());
	for c in value.chars() {
		match c {
			' ' => out.push('+'),
			'%' | '&' | '#' | '+' | '=' | '?' | ';' => {
				let mut buf = [0u8; 4];
				for b in c.encode_utf8(&mut buf).as_bytes() {
					out.push('%');
					out.push(char::from_digit(u32::from(*b) >> 4, 16).unwrap());
					out.push(char::from_digit(u32::from(*b) & 15, 16).unwrap());
				}
			}
			_ => out.push(c),
		}
	}
	out
}

/// Fetch a search page, logging the response status and page title —
/// anti-bot challenges (e.g. Cloudflare) return 200 with a challenge page
/// that parses to zero entries, and this makes that visible in the logs.
fn fetch_search_html(url: &str) -> Result<Document> {
	let response = Request::get(url)?.send()?;
	let status = response.status_code();
	let doc = response.get_html()?;
	match doc.select_first("title").and_then(|t| t.text()) {
		Some(title) => println!("search status={status} title={title:?}"),
		None => println!("search status={status} (no title)"),
	}
	Ok(doc)
}

/// Parse item entries (series/doujin/anthology links) from a search page.
fn parse_search_entries(html: &Document) -> Vec<Manga> {
	let mut entries: Vec<Manga> = Vec::new();
	if let Some(list) = html.select("dl.chapter-list dd") {
		for dd in list {
			let Some(link) = dd.select_first("a.name") else {
				continue;
			};
			let Some(href) = link.attr("href") else {
				continue;
			};
			// only item links; the unfiltered search also lists chapter-level
			// entries under /chapters/
			if !href.starts_with('/') || href.starts_with("/chapters/") {
				continue;
			}
			entries.push(Manga {
				key: String::from(&href[1..]),
				title: link.text().unwrap_or_default(),
				..Default::default()
			});
		}
	}
	entries
}

/// Whether the search page's pager offers a page beyond `page`.
fn has_next_search_page(html: &Document, page: i32) -> bool {
	let mut max_page = 0;
	if let Some(pager) = html.select(".pagination a") {
		for a in pager {
			if let Some(text) = a.text()
				&& let Ok(n) = text.trim().parse::<i32>()
			{
				max_page = max_page.max(n);
			}
		}
	}
	page < max_page
}

/// Map a set of tags to the site namespace its item lives under, preferring
/// Series > Anthology > Doujin (added.json chapters carry their parent as a tag).
fn classify(tags: &[Tag]) -> Option<(&'static str, String, String)> {
	for (tag_type, namespace) in [
		("Series", "series"),
		("Anthology", "anthologies"),
		("Doujin", "doujins"),
	] {
		if let Some(tag) = tags
			.iter()
			.find(|t| t.tag_type == tag_type && t.permalink.is_some())
		{
			return Some((
				namespace,
				tag.name.clone(),
				tag.permalink.clone().unwrap_or_default(),
			));
		}
	}
	None
}

/// Slugs look like `{series}_ch12` or `{series}_ch9_5`; decimals are encoded
/// with underscores. Returns None for oneshots and dated updates.
fn chapter_number_from_slug(slug: &str) -> Option<f32> {
	let rest = &slug[slug.rfind("_ch")? + 3..];
	let end = rest
		.find(|c: char| !(c.is_ascii_digit() || c == '_' || c == '.'))
		.unwrap_or(rest.len());
	let raw = rest[..end].trim_matches('_');
	if raw.is_empty() {
		return None;
	}
	let mut normalized = String::with_capacity(raw.len());
	for c in raw.chars() {
		normalized.push(if c == '_' { '.' } else { c });
	}
	normalized.parse::<f32>().ok()
}

/// `aliases` is observed as either a string or a string array.
fn aliases_list(value: &serde_json::Value) -> Option<Vec<String>> {
	match value {
		serde_json::Value::String(s) => Some(vec![s.clone()]),
		serde_json::Value::Array(items) => {
			let list: Vec<String> = items
				.iter()
				.filter_map(|v| v.as_str().map(String::from))
				.collect();
			(!list.is_empty()).then_some(list)
		}
		_ => None,
	}
}

fn strip_html(html: &str) -> Option<String> {
	Element::from(Html::parse_fragment(html).ok()?).text()
}

register_source!(Dynasty, ListingProvider, Home, DeepLinkHandler);

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn classify_prefers_series() {
		let tag = |name: &str, tag_type: &str| Tag {
			name: String::from(name),
			tag_type: String::from(tag_type),
			permalink: Some(String::from("x")),
		};
		let tags = vec![tag("Doujin Title", "Doujin"), tag("The Series", "Series")];
		let (namespace, name, _) = classify(&tags).unwrap();
		assert_eq!(namespace, "series");
		assert_eq!(name, "The Series");
	}

	#[aidoku_test]
	fn parses_chapter_numbers_from_slugs() {
		assert_eq!(chapter_number_from_slug("their_story_ch199"), Some(199.0));
		assert_eq!(chapter_number_from_slug("didnt_we_say_ch9_5"), Some(9.5));
		assert_eq!(chapter_number_from_slug("some_oneshot"), None);
	}

	#[aidoku_test]
	fn fetches_recently_added() {
		let source = Dynasty;
		let result = source.recently_added(1).unwrap();
		println!("entries: {}", result.entries.len());
		assert!(!result.entries.is_empty());
		assert!(result.has_next_page);
		// covers joined from the /chapters page
		assert!(result.entries.iter().any(|m| m.cover.is_some()));
	}

	#[aidoku_test]
	fn builds_home_layout() {
		let source = Dynasty;
		let home = source.get_home().unwrap();
		println!("components: {}", home.components.len());
		assert!(!home.components.is_empty());
	}

	#[aidoku_test]
	fn default_search_state_has_covers() {
		let source = Dynasty;
		let result = source.get_search_manga_list(None, 1, Vec::new()).unwrap();
		println!("entries: {}", result.entries.len());
		assert!(!result.entries.is_empty());
		assert!(result.entries.iter().any(|m| m.cover.is_some()));
	}

	#[aidoku_test]
	fn searches_series() {
		let source = Dynasty;
		let result = source
			.get_search_manga_list(Some(String::from("their story")), 1, Vec::new())
			.unwrap();
		println!("results: {}", result.entries.len());
		assert!(result.entries.iter().any(|m| m.key == "series/their_story"));
	}

	#[aidoku_test]
	fn searches_user_reported_query() {
		let source = Dynasty;
		let result = source
			.get_search_manga_list(
				Some(String::from(
					"that time i was blackmailed by the class green tea",
				)),
				1,
				Vec::new(),
			)
			.unwrap();
		println!(
			"results: {:?}",
			result.entries.iter().map(|m| &m.key).collect::<Vec<_>>()
		);
		assert!(result.entries.iter().any(|m| {
			m.key == "series/that_time_i_was_blackmailed_by_the_classs_green_tea_bitch"
		}));
	}

	#[aidoku_test]
	fn fetches_details_and_chapters() {
		let source = Dynasty;
		let manga = Manga {
			key: String::from("series/their_story"),
			..Default::default()
		};
		let updated = source.get_manga_update(manga, true, true).unwrap();
		println!(
			"title: {:?}, chapters: {}",
			updated.title,
			updated.chapters.as_ref().map(|c| c.len()).unwrap_or(0)
		);
		assert_eq!(updated.title, "Their Story");
		assert_eq!(updated.status, MangaStatus::Ongoing);
		// their_story carries no NSFW/Ecchi tag and is marked "Read left to right"
		assert_eq!(updated.content_rating, ContentRating::Safe);
		assert_eq!(updated.viewer, Viewer::LeftToRight);
		let chapters = updated.chapters.unwrap();
		assert!(chapters.len() >= 70);
	}

	#[aidoku_test]
	fn fetches_pages() {
		let source = Dynasty;
		let manga = Manga {
			key: String::from("series/their_story"),
			..Default::default()
		};
		let updated = source.get_manga_update(manga, false, true).unwrap();
		let chapter = updated.chapters.unwrap().pop().unwrap();
		let pages = source.get_page_list(Manga::default(), chapter).unwrap();
		println!("pages: {}", pages.len());
		assert!(!pages.is_empty());
	}
}
