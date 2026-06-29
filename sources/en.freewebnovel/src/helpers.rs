use crate::BASE_URL;
use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, Result,
	alloc::{String, Vec, string::ToString},
	imports::{
		html::{Document, Element, Html, Kind},
		net::Request,
	},
	prelude::*,
};
use serde::Deserialize;

/// Maximum chapters the AJAX chapter-list endpoint returns per request.
/// Larger values are clamped to this server-side.
const CHAPTER_PAGE_SIZE: i32 = 200;

pub fn request_html(url: &str) -> Result<Document> {
	Ok(Request::get(url)?.html()?)
}

pub fn build_novel_url(slug: &str) -> String {
	format!("{BASE_URL}/novel/{slug}")
}

pub fn build_chapter_url(slug: &str, chapter_key: &str) -> String {
	format!("{BASE_URL}/novel/{slug}/{chapter_key}")
}

/// Normalizes cover image URLs by replacing "ss.jpg" with "s.jpg" to get a higher resolution image.
fn normalize_cover_url(url: &str) -> Option<String> {
	let url = url.trim();
	(!url.is_empty()).then(|| url.replace("ss.jpg", "s.jpg"))
}

pub fn build_sort_url(kind: &str, page: i32) -> String {
	if page <= 1 {
		format!("{BASE_URL}/sort/{kind}")
	} else {
		format!("{BASE_URL}/sort/{kind}/{page}")
	}
}

pub fn has_next_page(html: &Document, kind: &str, page: i32) -> bool {
	html.select_first(format!("a[href*='/sort/{kind}/{}']", page + 1))
		.is_some()
}

pub fn parse_novel_and_chapter(url: &str) -> Option<(String, Option<String>)> {
	let path = url
		.rsplit("freewebnovel.com")
		.next()
		.unwrap_or(url)
		.trim_start_matches('/');
	let mut parts = path.split('/');
	if parts.next()? != "novel" {
		return None;
	}
	let slug = parts.next()?.to_string();
	if slug.is_empty() {
		return None;
	}
	let chapter_key = parts
		.next()
		.and_then(|seg| seg.starts_with("chapter-").then(|| seg.to_string()));
	Some((slug, chapter_key))
}

/// Parse the leading chapter number from a title like "Chapter 12.5: ...".
/// Returns `None` when the title isn't a "Chapter <number>" label.
pub fn parse_chapter_number(name: &str) -> Option<f32> {
	let digits = name.trim().strip_prefix("Chapter")?.trim_start();
	let end = digits
		.find(|c: char| !c.is_ascii_digit() && c != '.')
		.unwrap_or(digits.len());
	digits[..end].parse().ok()
}

pub fn content_rating_from_tags(tags: &[String]) -> ContentRating {
	const NSFW_TAGS: &[&str] = &["Adult", "Mature"];
	const LITE_TAGS: &[&str] = &["Smut", "Ecchi", "Yaoi", "Yuri"];
	if tags.iter().any(|tag| NSFW_TAGS.contains(&tag.as_str())) {
		ContentRating::NSFW
	} else if tags.iter().any(|tag| LITE_TAGS.contains(&tag.as_str())) {
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	}
}

/// Response shape of the AJAX chapter-list endpoint
/// (`/novel/{slug}?ajax=chapters&page=N&pageSize=N`).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChapterListResponse {
	/// `<li>` chapter items for the requested page, as an HTML fragment.
	html: String,
	/// Total number of chapter pages for the current `pageSize`.
	total_page: i32,
	/// Total number of chapters across every page.
	total_chapters: i32,
}

/// Fetch a single page of the chapter list through the AJAX endpoint.
fn fetch_chapter_page(slug: &str, page: i32) -> Result<ChapterListResponse> {
	let url =
		format!("{BASE_URL}/novel/{slug}?ajax=chapters&page={page}&pageSize={CHAPTER_PAGE_SIZE}");
	Request::get(&url)?
		.header("X-Requested-With", "XMLHttpRequest")
		.json_owned()
}

/// Extract every chapter for a novel.
///
/// The novel page only renders the first page (40 chapters) of the chapter
/// list; the remaining chapters are loaded on demand through an AJAX endpoint
/// that reports the total number of pages. Walk every page so the full list is
/// returned, newest chapter first.
pub fn extract_chapters(slug: &str) -> Result<Vec<Chapter>> {
	let first = fetch_chapter_page(slug, 1)?;
	let mut chapters = Vec::with_capacity(first.total_chapters.max(0) as usize);
	collect_chapter_items(&first.html, &mut chapters);
	for page in 2..=first.total_page.max(1) {
		let response = fetch_chapter_page(slug, page)?;
		collect_chapter_items(&response.html, &mut chapters);
	}
	// The endpoint lists chapters oldest-first across pages; reverse so the
	// newest chapter comes first.
	chapters.reverse();
	Ok(chapters)
}

/// Parse an HTML fragment of `<li>` chapter items and append them to `chapters`.
fn collect_chapter_items(html: &str, chapters: &mut Vec<Chapter>) {
	let Ok(document) = Html::parse_fragment_with_url(html, BASE_URL) else {
		return;
	};
	let Some(items) = document.select("li") else {
		return;
	};
	for item in items {
		if let Some(chapter) = parse_chapter_item(&item) {
			chapters.push(chapter);
		}
	}
}

/// Strip a leading "Chapter N" label and its `:`/`-` separator from a title,
/// leaving just the subtitle. Returns `None` when the title doesn't start with
/// that exact label, so the caller can keep the original title.
fn strip_chapter_label(title: &str, number: f32) -> Option<String> {
	let rest = title.strip_prefix(&format!("Chapter {number}"))?.trim();
	let rest = rest.strip_prefix([':', '-']).unwrap_or(rest).trim();
	Some(rest.to_string())
}

/// Build a [`Chapter`] from a single `<li>` chapter item.
fn parse_chapter_item(item: &Element) -> Option<Chapter> {
	let link = item.select_first("a[href]")?;
	let url = link.attr("abs:href")?;
	let (_, chapter_key) = parse_novel_and_chapter(&url)?;
	let chapter_key = chapter_key?;

	let raw_title = link.text()?;
	let chapter_number = parse_chapter_number(&raw_title);
	let title = chapter_number
		.and_then(|number| strip_chapter_label(&raw_title, number))
		.unwrap_or(raw_title);

	Some(Chapter {
		key: chapter_key,
		title: (!title.is_empty()).then_some(title),
		chapter_number,
		url: Some(url),
		..Default::default()
	})
}

fn convert_element_to_markdown(element: &Element, output: &mut String) {
	let nodes = element.child_nodes();

	for node in nodes {
		match node.kind() {
			Kind::TextNode => {
				if let Some(text) = node.text() {
					if text.len() >= 3 && text.replace("-", "").trim().is_empty() {
						output.push_str(&text);
					} else {
						output.push_str(&text.replace("*", r"\*").replace("-", r"\-"));
					}
				}
			}
			Kind::Element => {
				let el = Element::try_from(node).unwrap();
				convert_tag_to_markdown(&el, output);
			}
			_ => (),
		}
	}
}

fn convert_tag_to_markdown(element: &Element, output: &mut String) {
	let tag = element.tag_name().unwrap_or_default();

	match tag.as_str() {
		"p" => {
			convert_element_to_markdown(element, output);
			output.push_str("\n\n");
		}
		"br" => {
			output.push_str("  \n");
		}
		"h1" => {
			output.push_str("# ");
			convert_element_to_markdown(element, output);
			output.push_str("\n\n");
		}
		"h2" => {
			output.push_str("## ");
			convert_element_to_markdown(element, output);
			output.push_str("\n\n");
		}
		"h3" => {
			output.push_str("### ");
			convert_element_to_markdown(element, output);
			output.push_str("\n\n");
		}
		"h4" => {
			output.push_str("#### ");
			convert_element_to_markdown(element, output);
			output.push_str("\n\n");
		}
		"h5" => {
			output.push_str("##### ");
			convert_element_to_markdown(element, output);
			output.push_str("\n\n");
		}
		"h6" => {
			output.push_str("###### ");
			convert_element_to_markdown(element, output);
			output.push_str("\n\n");
		}
		"strong" | "b" => {
			output.push_str("**");
			convert_element_to_markdown(element, output);
			output.push_str("**");
		}
		"em" | "i" => {
			output.push('*');
			convert_element_to_markdown(element, output);
			output.push('*');
		}
		"u" => {
			output.push_str("__");
			convert_element_to_markdown(element, output);
			output.push_str("__");
		}
		"s" | "strike" | "del" => {
			output.push_str("~~");
			convert_element_to_markdown(element, output);
			output.push_str("~~");
		}
		_ => {
			convert_element_to_markdown(element, output);
		}
	}
}

pub fn extract_chapter_text(html: &Document) -> Result<String> {
	let mut text = String::new();

	if let Some(container) = html.select_first("#article") {
		// Remove ADs
		if let Some(ads) = container.select("subtxt") {
			ads.for_each(Element::remove);
		}
		if let Some(things) = container.select("div") {
			things.for_each(Element::remove);
		}
		convert_element_to_markdown(&container, &mut text);
		text = text.replace("****", "");
	}
	if text.is_empty() {
		bail!("chapter text not found");
	}
	Ok(text.to_string())
}

pub fn parse_search_results(html: &Document) -> Vec<Manga> {
	let mut entries = Vec::new();
	if let Some(els) = html.select("div.pic > a[href*='/novel/']") {
		append_anchor_entries(els, &mut entries);
	}
	entries
}

pub fn parse_hot_entries(html: &Document) -> Vec<Manga> {
	let mut entries = Vec::new();
	if let Some(container) = html.select_first("div.m-book")
		&& let Some(anchors) = container.select("div.pic > a[href*='/novel/']")
	{
		append_anchor_entries(anchors, &mut entries);
	}
	entries
}

pub fn parse_home_section(html: &Document, heading: &str) -> Vec<Manga> {
	find_section_container(html, heading).map_or_else(Vec::new, |container| {
		parse_entries_from_container(&container)
	})
}

fn parse_entries_from_container(root: &Element) -> Vec<Manga> {
	let mut entries = Vec::new();
	if let Some(els) =
		root.select("div.pic ~ a[href*='/novel/'], div.rec div.pic > a[href*='/novel/']")
	{
		append_anchor_entries(els, &mut entries);
	}
	entries
}

fn find_section_container(html: &Document, heading: &str) -> Option<Element> {
	let selector = format!("div > div h3:contains({heading})");
	html.select_first(&selector)
		.and_then(|h| h.parent())
		.and_then(|p| p.parent())
}

fn append_anchor_entries<I>(anchors: I, entries: &mut Vec<Manga>)
where
	I: Iterator<Item = Element>,
{
	for el in anchors {
		let Some(url) = el.attr("abs:href") else {
			continue;
		};
		let Some((slug, _)) = parse_novel_and_chapter(&url) else {
			continue;
		};
		let Some(title) = extract_anchor_title(&el) else {
			continue;
		};
		let cover = el.parent().and_then(|p| find_cover_image(&p));
		let manga = Manga {
			key: slug,
			title,
			cover,
			url: Some(url),
			..Default::default()
		};
		entries.push(manga);
	}
}

fn extract_anchor_title(el: &Element) -> Option<String> {
	if let Some(tags) = el.select(".new, .hot") {
		for tag in tags {
			tag.remove();
		}
	}
	let title = el
		.text() // Used by Homepage sections
		.or_else(|| el.select_first("img").and_then(|img| img.attr("alt"))); // Used by search results
	title.and_then(|t| {
		let trimmed = t.trim();
		(!trimmed.is_empty()).then(|| trimmed.to_string())
	})
}
enum MetaSelector {
	Title,
	Cover,
	Authors,
	Description,
	Url,
	Tags,
	Status,
}
pub fn fill_manga_details(html: &Document, mut manga: Manga) -> Result<Manga> {
	let Some(title) = get_meta_data(html, MetaSelector::Title) else {
		bail!("Title not found");
	};
	manga.title = title;
	manga.cover = get_meta_data(html, MetaSelector::Cover);
	manga.url = get_meta_data(html, MetaSelector::Url);
	if let Some(parts) = html.select("h4.abstract + div.txt p") {
		let description = extract_text_from_elements(parts);
		if !description.is_empty() {
			manga.description = Some(description);
		}
	} else {
		manga.description = get_meta_data(html, MetaSelector::Description);
	}

	manga.authors = get_meta_data(html, MetaSelector::Authors)
		.map(|authors| authors.split(',').map(|s| s.trim().to_string()).collect());
	manga.tags = get_meta_data(html, MetaSelector::Tags)
		.map(|tags| tags.split(',').map(|s| s.trim().to_string()).collect());
	manga.content_rating = manga
		.tags
		.as_deref()
		.map(content_rating_from_tags)
		.unwrap_or(ContentRating::Unknown);
	manga.status = match get_meta_data(html, MetaSelector::Status).as_deref() {
		Some("OnGoing") => MangaStatus::Ongoing,
		Some("Completed") => MangaStatus::Completed,
		_ => MangaStatus::Unknown,
	};
	Ok(manga)
}

fn get_meta_data(html: &Document, selector: MetaSelector) -> Option<String> {
	let query = match selector {
		MetaSelector::Title => "meta[property='og:title']",
		MetaSelector::Description => "meta[property='og:description']",
		MetaSelector::Cover => "meta[property='og:image']",
		MetaSelector::Authors => "meta[property='og:novel:author']",
		MetaSelector::Tags => "meta[property='og:novel:genre']",
		MetaSelector::Url => "meta[property='og:url']",
		MetaSelector::Status => "meta[property='og:novel:status']",
	};
	html.select_first(query)
		.and_then(|el| el.attr("content"))
		.filter(|s| !s.trim().is_empty())
		.map(|s| s.trim().to_string())
}

fn extract_text_from_elements<I>(elements: I) -> String
where
	I: Iterator<Item = Element>,
{
	elements
		.filter_map(|el| el.text())
		.collect::<Vec<_>>()
		.join("\n")
}

fn find_cover_image(el: &Element) -> Option<String> {
	let cover = el
		.select_first("div.pic > a > img")
		.and_then(|img| img.attr("abs:src"))
		.or_else(|| el.select_first("img").and_then(|img| img.attr("abs:src")));
	cover.and_then(|url| normalize_cover_url(&url))
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn parses_chapter_number() {
		assert_eq!(parse_chapter_number("Chapter 40: Foo"), Some(40.0));
		assert_eq!(parse_chapter_number("Chapter 12.5"), Some(12.5));
		assert_eq!(parse_chapter_number("Prologue"), None);
		assert_eq!(parse_chapter_number("Chapter"), None);
	}

	#[aidoku_test]
	fn strips_chapter_label() {
		assert_eq!(
			strip_chapter_label("Chapter 40: Foo", 40.0).as_deref(),
			Some("Foo")
		);
		assert_eq!(
			strip_chapter_label("Chapter 7 - Bar", 7.0).as_deref(),
			Some("Bar")
		);
		// A label with no subtitle collapses to an empty string.
		assert_eq!(strip_chapter_label("Chapter 40", 40.0).as_deref(), Some(""));
		// A formatting mismatch keeps the caller's original title.
		assert_eq!(strip_chapter_label("Chapter 07", 7.0), None);
	}
}
