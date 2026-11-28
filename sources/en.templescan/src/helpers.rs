use crate::{
	rsc::{Classified, Flight},
	tokens,
};
use aidoku::{
	Chapter, ContentRating, Manga, MangaPageResult, MangaStatus, Result, Viewer,
	alloc::{
		String, Vec,
		collections::{BTreeMap, BTreeSet},
		format,
		string::ToString,
	},
	helpers::string::PlainText,
	imports::{
		html::{Document, Element, Html},
		net::Request,
		std::parse_date_with_options,
	},
	prelude::*,
};
use serde_json::{Map, Value};

pub const BASE_URL: &str = "https://templetoons.com";
const PAGE_SIZE: usize = 20;

const CATALOG_KNOWN: [&str; 7] = [
	"thumbnail",
	"badge",
	"status",
	"created_at",
	"alternative_names",
	"update_chapter",
	"total_views",
];

pub struct CatalogItem {
	pub slug: String,
	pub title: String,
	pub cover: Option<String>,
	pub badge: Option<String>,
	pub status: Option<String>,
	pub alternative_names: Option<String>,
	pub created_at: Option<i64>,
	pub updated_at: Option<i64>,
	pub views: u64,
}

pub struct ReaderData {
	pub tokens: Vec<String>,
	pub script: Option<(u64, String)>,
}

pub fn clean(text: &str) -> String {
	text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn image_url(path: Option<&str>) -> Option<String> {
	let path = path?.trim();
	if path.is_empty() {
		return None;
	}
	Some(
		if path.starts_with("http://") || path.starts_with("https://") {
			path.to_string()
		} else if let Some(rest) = path.strip_prefix("//") {
			format!("https://{rest}")
		} else if path.starts_with('/') {
			format!("{BASE_URL}{path}")
		} else {
			format!("{BASE_URL}/{path}")
		},
	)
}

fn parse_status(status: Option<&str>) -> MangaStatus {
	match status.unwrap_or_default().to_ascii_lowercase().as_str() {
		"ongoing" => MangaStatus::Ongoing,
		"completed" => MangaStatus::Completed,
		"hiatus" => MangaStatus::Hiatus,
		"canceled" | "cancelled" | "dropped" => MangaStatus::Cancelled,
		_ => MangaStatus::Unknown,
	}
}

pub fn rating_of(label: &str) -> ContentRating {
	let lower = label.to_ascii_lowercase();
	if lower.contains("+18")
		|| lower.contains("18+")
		|| lower.contains("adult")
		|| lower.contains("smut")
		|| lower.contains("hentai")
	{
		ContentRating::NSFW
	} else if lower.contains("mature") || lower.contains("ecchi") {
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	}
}

pub fn strip_html(html: &str) -> String {
	Html::parse_fragment(format!("<div>{html}</div>"))
		.ok()
		.and_then(|doc| doc.select_first("div")?.text())
		.unwrap_or_else(|| html.into())
}

fn parse_iso_utc(value: Option<&str>) -> Option<i64> {
	parse_date_with_options(value?, "yyyy-MM-dd'T'HH:mm:ss.SSSXXX", "en_US_POSIX", "UTC")
}

fn first_number(text: &str) -> Option<f32> {
	let mut digits = String::new();
	let mut seen_dot = false;
	for ch in text.chars() {
		if ch.is_ascii_digit() {
			digits.push(ch);
		} else if ch == '.' && !seen_dot && !digits.is_empty() {
			seen_dot = true;
			digits.push(ch);
		} else if !digits.is_empty() {
			break;
		}
	}
	digits.trim_end_matches('.').parse().ok()
}

fn strip_chapter_label(raw: &str, number: Option<f32>) -> Option<String> {
	let trimmed = raw.trim();
	if trimmed.is_empty() {
		return None;
	}
	let Some(num) = number else {
		return Some(trimmed.into());
	};
	if trimmed
		.parse::<f32>()
		.ok()
		.is_some_and(|n| (n - num).abs() < f32::EPSILON)
	{
		return None;
	}
	let rest = ["chapter", "ch.", "ch ", "episode", "ep.", "#"]
		.iter()
		.find_map(|prefix| {
			let (head, tail) = trimmed.split_at_checked(prefix.len())?;
			head.eq_ignore_ascii_case(prefix).then_some(tail)
		})
		.map(str::trim_start);
	if let Some(after_prefix) = rest {
		let after_num = after_prefix.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.');
		let num_str = after_prefix
			.strip_suffix(after_num)
			.unwrap_or("")
			.trim_end_matches('.');
		if num_str
			.parse::<f32>()
			.ok()
			.is_some_and(|n| (n - num).abs() < f32::EPSILON)
		{
			let tail = after_num
				.trim_start_matches(|c: char| {
					c.is_whitespace() || matches!(c, ':' | '-' | '–' | '—' | '|')
				})
				.trim();
			return (!tail.is_empty()).then(|| tail.into());
		}
	}
	Some(trimmed.into())
}

pub fn parse_comic_href(href: &str) -> Option<(String, Option<String>)> {
	let after = href.split("/comic/").nth(1)?;
	let mut parts = after.split(['/', '?', '#']);
	let slug = parts.next().filter(|s| !s.is_empty())?;
	let chapter = parts.next().filter(|s| !s.is_empty());
	Some((slug.into(), chapter.map(String::from)))
}

// The site serves its own check page, with a 200, in place of the page it was asked for.
pub fn is_challenge_page(doc: &Document) -> bool {
	doc.select_first("script[src*='/app/challenge/page-']")
		.is_some()
}

// Without the headers a browser sends when it opens a page, the site redirects to its check page.
pub fn build_request(url: &str) -> Result<Request> {
	Ok(Request::get(url)?
		.header("Referer", &format!("{BASE_URL}/"))
		.header(
			"Accept",
			"text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
		)
		.header("Sec-Fetch-Dest", "document")
		.header("Sec-Fetch-Mode", "navigate")
		.header("Sec-Fetch-Site", "none"))
}

pub fn fetch_doc(url: &str) -> Result<Document> {
	let response = build_request(url)?.send()?;
	let status = response.status_code();
	if !(200..300).contains(&status) {
		bail!("Temple Scan answered with HTTP {status}");
	}
	let doc = response.get_html()?;
	if is_challenge_page(&doc) {
		bail!("Temple Scan answered with its verification page; try again later");
	}
	Ok(doc)
}

fn parse_views(val: Option<&Value>) -> u64 {
	match val {
		Some(Value::Number(n)) => n
			.as_u64()
			.or_else(|| n.as_f64().filter(|f| *f >= 0.0).map(|f| f as u64))
			.unwrap_or(0),
		Some(Value::String(s)) => s.trim().parse().unwrap_or(0),
		_ => 0,
	}
}

fn parse_catalog_records(records: &[&Map<String, Value>]) -> Vec<CatalogItem> {
	let Some(fields) = Classified::of(records, &CATALOG_KNOWN) else {
		return Vec::new();
	};
	let mut seen = BTreeSet::new();
	records
		.iter()
		.filter_map(|record| {
			let slug = record.get(&fields.slug)?.as_str()?.trim();
			let title = record.get(&fields.text)?.as_str()?.trim();
			if slug.is_empty() || title.is_empty() || !seen.insert(slug) {
				return None;
			}
			Some(CatalogItem {
				slug: slug.into(),
				title: title.into(),
				cover: image_url(record.get("thumbnail").and_then(Value::as_str)),
				badge: record
					.get("badge")
					.and_then(Value::as_str)
					.map(|s| s.trim().into()),
				status: record
					.get("status")
					.and_then(Value::as_str)
					.map(|s| s.trim().into()),
				alternative_names: record
					.get("alternative_names")
					.and_then(Value::as_str)
					.map(String::from),
				created_at: parse_iso_utc(record.get("created_at").and_then(Value::as_str)),
				updated_at: parse_iso_utc(record.get("update_chapter").and_then(Value::as_str)),
				views: parse_views(record.get("total_views")),
			})
		})
		.collect()
}

pub fn parse_catalog(doc: &Document) -> Vec<CatalogItem> {
	let flight = Flight::from_document(doc);
	if let Some(props) = flight.find(&["list", "initialPage"])
		&& let Some(list) = props.get("list").and_then(Value::as_array)
	{
		let records: Vec<&Map<String, Value>> = list.iter().filter_map(Value::as_object).collect();
		let parsed = parse_catalog_records(&records);
		if !parsed.is_empty() {
			return parsed;
		}
	}
	let mut best = Vec::new();
	for array in flight.all_arrays() {
		let records: Vec<&Map<String, Value>> = array.iter().filter_map(Value::as_object).collect();
		if records.len() <= best.len()
			|| !records.first().is_some_and(|r| r.contains_key("thumbnail"))
		{
			continue;
		}
		let parsed = parse_catalog_records(&records);
		if parsed.len() > best.len() {
			best = parsed;
		}
	}
	best
}

pub fn sort_catalog(items: &mut [CatalogItem], sort: i32, ascending: bool) {
	items.sort_unstable_by(|a, b| {
		let ord = match sort {
			1 => a
				.created_at
				.unwrap_or(i64::MIN)
				.cmp(&b.created_at.unwrap_or(i64::MIN)),
			2 => a
				.title
				.bytes()
				.map(|c| c.to_ascii_lowercase())
				.cmp(b.title.bytes().map(|c| c.to_ascii_lowercase())),
			3 => a.views.cmp(&b.views),
			_ => a
				.updated_at
				.unwrap_or(i64::MIN)
				.cmp(&b.updated_at.unwrap_or(i64::MIN)),
		}
		.then_with(|| a.slug.cmp(&b.slug));
		if ascending { ord } else { ord.reverse() }
	});
}

pub fn page_catalog(items: Vec<CatalogItem>, page: i32) -> MangaPageResult {
	let start = (page.max(1) as usize - 1) * PAGE_SIZE;
	let has_next_page = items.len() > start + PAGE_SIZE;
	MangaPageResult {
		entries: items
			.into_iter()
			.skip(start)
			.take(PAGE_SIZE)
			.map(|item| Manga {
				url: Some(format!("{BASE_URL}/comic/{}", item.slug)),
				key: item.slug,
				title: item.title,
				cover: item.cover,
				status: parse_status(item.status.as_deref()),
				content_rating: item
					.badge
					.as_deref()
					.map(rating_of)
					.unwrap_or(ContentRating::Safe),
				..Default::default()
			})
			.collect(),
		has_next_page,
	}
}

fn is_badge(label: &str) -> bool {
	["new", "premium", "up next", "read"]
		.iter()
		.any(|badge| label.eq_ignore_ascii_case(badge))
}

pub fn parse_chapter_anchor(
	anchor: &Element,
	series: &str,
	chapter_slug: String,
	position: Option<f32>,
	include_url: bool,
) -> Chapter {
	let mut labels: Vec<String> = Vec::new();
	let mut premium = false;
	for span in anchor.select("span").into_iter().flatten() {
		let label = span.own_text().map(|t| clean(&t)).unwrap_or_default();
		if label.eq_ignore_ascii_case("premium") {
			premium = true;
		}
		if !label.is_empty() && !is_badge(&label) && !span.has_class("sr-only") {
			labels.push(label);
		}
	}
	let name = labels
		.first()
		.cloned()
		.unwrap_or_else(|| chapter_slug.clone());
	let number = first_number(&name).or(position);
	let subtitle = labels
		.get(1)
		.filter(|s| {
			anchor
				.select_first("time")
				.and_then(|t| t.text())
				.is_none_or(|dt| !dt.contains(s.as_str()))
		})
		.and_then(|s| strip_chapter_label(s, number));
	let title = subtitle.or_else(|| strip_chapter_label(&name, number));
	let date_uploaded = anchor
		.select_first("time[datetime]")
		.and_then(|el| el.attr("datetime"))
		.and_then(|dt| parse_iso_utc(Some(&dt)));
	Chapter {
		url: include_url.then(|| format!("{BASE_URL}/comic/{series}/{chapter_slug}")),
		key: chapter_slug,
		title,
		chapter_number: number,
		date_uploaded,
		locked: premium,
		..Default::default()
	}
}

fn series_title(doc: &Document) -> Option<String> {
	doc.select_first("#series-title")?
		.text()
		.filter(|t| !t.is_empty())
}

fn series_cover(doc: &Document) -> Option<String> {
	image_url(
		doc.select_first("meta[property='og:image']")?
			.attr("content")
			.as_deref(),
	)
}

pub fn series_card(doc: &Document, key: String) -> Option<Manga> {
	Some(Manga {
		url: Some(format!("{BASE_URL}/comic/{key}")),
		title: series_title(doc)?,
		cover: series_cover(doc),
		key,
		..Default::default()
	})
}

pub fn parse_series_details(doc: &Document, manga: &mut Manga) -> Result<()> {
	manga.title = series_title(doc).ok_or_else(|| error!("Title not found"))?;
	manga.cover = series_cover(doc).or_else(|| manga.cover.take());

	if let Some(synopsis) = doc.select_first("#series-synopsis-text") {
		let paragraphs: Vec<String> = synopsis
			.select("p")
			.into_iter()
			.flatten()
			.filter_map(|p| p.text())
			.filter(|t| !t.is_empty())
			.collect();
		let text = if !paragraphs.is_empty() {
			paragraphs.join("\n\n")
		} else {
			synopsis.text().map(|t| strip_html(&t)).unwrap_or_default()
		};
		if !text.is_empty() {
			manga.description = Some(text.escape_markdown());
		}
	}

	for entry in doc.select("dl > div").into_iter().flatten() {
		if entry
			.select_first("dt")
			.and_then(|el| el.text())
			.is_some_and(|t| t.eq_ignore_ascii_case("author"))
			&& let Some(val) = entry.select_first("dd").and_then(|el| el.text())
		{
			let mut seen = BTreeSet::new();
			let authors: Vec<String> = val
				.split([',', '/', '|', ';'])
				.map(str::trim)
				.filter(|name| !name.is_empty() && seen.insert(*name))
				.map(String::from)
				.collect();
			manga.authors = (!authors.is_empty()).then_some(authors);
		}
	}

	for stat in doc
		.select("ul[aria-label='Series stats'] > li")
		.into_iter()
		.flatten()
	{
		if let Some(text) = stat.text() {
			let parsed = parse_status(Some(&text));
			if parsed != MangaStatus::Unknown {
				manga.status = parsed;
				break;
			}
		}
	}

	let mut tags: Vec<String> = doc
		.select("ul[aria-label='Genres'] > li")
		.into_iter()
		.flatten()
		.filter_map(|el| el.text())
		.filter(|t| !t.is_empty())
		.collect();
	tags.sort_unstable();
	tags.dedup();
	let mut content_rating = ContentRating::Safe;
	for tag in &tags {
		match rating_of(tag) {
			ContentRating::NSFW => {
				content_rating = ContentRating::NSFW;
				break;
			}
			ContentRating::Suggestive => content_rating = ContentRating::Suggestive,
			_ => {}
		}
	}
	manga.content_rating = content_rating;
	manga.tags = (!tags.is_empty()).then_some(tags);
	manga.viewer = Viewer::Webtoon;
	manga.url = Some(format!("{BASE_URL}/comic/{}", manga.key));
	Ok(())
}

fn chapter_positions(doc: &Document) -> BTreeMap<String, f32> {
	let mut map = BTreeMap::new();
	let flight = Flight::from_document(doc);
	let Some(seasons) = flight
		.find(&["seriesData"])
		.and_then(|props| props.get("seriesData"))
		.and_then(Value::as_object)
		.and_then(|series| series.values().find_map(Value::as_array))
	else {
		return map;
	};
	let known = ["index", "chapter_title", "chapter_thumbnail", "created_at"];
	for season in seasons.iter().filter_map(Value::as_object) {
		let Some(chapters) = season.values().find_map(Value::as_array) else {
			continue;
		};
		let records: Vec<&Map<String, Value>> =
			chapters.iter().filter_map(Value::as_object).collect();
		let Some(fields) = Classified::of(&records, &known) else {
			continue;
		};
		for record in records {
			let Some(slug) = record.get(&fields.slug).and_then(Value::as_str) else {
				continue;
			};
			let pos = match record.get("index") {
				Some(Value::String(s)) => s.trim().parse::<f32>().ok(),
				Some(Value::Number(n)) => n.as_f64().map(|f| f as f32),
				_ => None,
			};
			if let Some(p) = pos.filter(|p| p.is_finite()) {
				map.insert(slug.into(), p);
			}
		}
	}
	map
}

pub fn parse_series_chapters(doc: &Document, slug: &str, show_locked: bool) -> Vec<Chapter> {
	let positions = chapter_positions(doc);
	let mut seen = BTreeSet::new();
	doc.select("#chapter-list > li")
		.into_iter()
		.flatten()
		.filter_map(|item| {
			let anchor = item.select_first("a[href]")?;
			let (owner, Some(ch_slug)) = parse_comic_href(&anchor.attr("href")?)? else {
				return None;
			};
			if owner != slug || !seen.insert(ch_slug.clone()) {
				return None;
			}
			let pos = positions.get(&ch_slug).copied();
			let chapter = parse_chapter_anchor(&anchor, slug, ch_slug, pos, true);
			(show_locked || !chapter.locked).then_some(chapter)
		})
		.collect()
}

pub fn decode_reader_tokens(tokens_list: &[String], seed: &str) -> Option<Vec<String>> {
	let mut urls = Vec::with_capacity(tokens_list.len());
	for (index, token) in tokens_list.iter().enumerate() {
		let url = if token.starts_with("https://") || token.starts_with("http://") {
			token.clone()
		} else {
			tokens::decode(token, index, seed)?
		};
		if !(url.starts_with("https://") || url.starts_with("http://")) {
			return None;
		}
		urls.push(url);
	}
	Some(urls)
}

pub fn extract_reader_tokens(doc: &Document) -> Option<ReaderData> {
	let flight = Flight::from_document(doc);
	let found = flight.all(&["layoutHints"]);
	let [props] = found.as_slice() else {
		return None;
	};
	let hints = props.get("layoutHints")?.as_array()?;
	let mut lists = props.iter().filter_map(|(key, value)| match value {
		Value::Array(items)
			if key != "layoutHints"
				&& items.len() == hints.len()
				&& items.iter().all(Value::is_string) =>
		{
			Some(items)
		}
		_ => None,
	});
	let (Some(list), None) = (lists.next(), lists.next()) else {
		return None;
	};
	let tokens: Vec<String> = list
		.iter()
		.filter_map(Value::as_str)
		.map(String::from)
		.collect();
	let script = flight.reference_of(props).and_then(|ref_id| {
		let parts = flight.module(ref_id)?.as_array()?;
		let module = parts.first()?.as_u64()?;
		let chunk = parts.get(1)?.as_array()?.last()?.as_str()?;
		(chunk.starts_with("static/chunks/") && chunk.ends_with(".js") && !chunk.contains(".."))
			.then(|| (module, chunk.into()))
	});
	Some(ReaderData { tokens, script })
}
