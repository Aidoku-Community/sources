use crate::BASE_URL;
use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, Result,
	alloc::{String, Vec, string::ToString},
	imports::{
		html::{Document, Element, Kind},
		net::Request,
		std::sleep,
	},
	prelude::*,
};

/// Fetches a URL as an HTML document, transparently retrying on HTTP 429.
///
/// The site rate-limits bursts (~15 requests) with a `429 Too Many Requests`
/// and a `Retry-After` header (typically 10s). Fetching a long novel's chapter
/// list needs many sequential page requests, so we honor `Retry-After` and
/// retry rather than silently dropping chapters.
pub fn request_html(url: &str) -> Result<Document> {
	const MAX_RETRIES: u32 = 3;
	const DEFAULT_RETRY_AFTER: i32 = 10;
	let mut attempt = 0;
	loop {
		let response = Request::get(url)?.send()?;
		if response.status_code() == 429 && attempt < MAX_RETRIES {
			let wait = response
				.get_header("retry-after")
				.and_then(|v| v.parse::<i32>().ok())
				.unwrap_or(DEFAULT_RETRY_AFTER)
				.clamp(1, 30);
			sleep(wait);
			attempt += 1;
			continue;
		}
		return Ok(response.get_html()?);
	}
}

/// Maximum chapters the site returns per page; larger `pageSize` values clamp
/// to this. Requesting the max minimizes the number of chapter-list requests
/// (keeping even long novels under the rate-limit threshold).
const CHAPTERS_PER_PAGE: usize = 200;

pub fn build_novel_url(slug: &str) -> String {
	format!("{BASE_URL}/novel/{slug}?pageSize={CHAPTERS_PER_PAGE}")
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

pub fn parse_chapter_number(name: &str) -> Option<f32> {
	let mut chapter = None;
	let mut name = name.trim();
	if name.starts_with("Chapter") {
		name = name[7..].trim_start();
		let bytes = name.as_bytes();
		let mut ch_end = 0;
		while ch_end < bytes.len()
			&& ((bytes[ch_end] as char).is_ascii_digit() || (bytes[ch_end] as char) == '.')
		{
			ch_end += 1;
		}
		if ch_end > 0
			&& let Ok(c) = name[..ch_end].parse::<f32>()
		{
			chapter = Some(c);
		}
	}
	chapter
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

/// Total number of chapters, parsed from the index `<select>`.
///
/// The `<select id="indexselect">` lists chapter ranges in 40-chapter chunks
/// (e.g. last option `"C.801 - C.809"`), regardless of the requested
/// `pageSize`. The last option's trailing number is the total chapter count.
/// Returns `None` for short novels that have no select (single page).
pub fn total_chapter_count(html: &Document) -> Option<usize> {
	let last = html.select("select#indexselect option")?.next_back()?;
	let text = last.text()?;
	text.split(|c: char| !c.is_ascii_digit())
		.rfind(|s| !s.is_empty())
		.and_then(|n| n.parse().ok())
}

/// Parse a single chapter `<li>` element into a [Chapter].
fn chapter_from_item(item: &Element) -> Option<Chapter> {
	let link = item.select_first("a[href]")?;
	let url = link.attr("abs:href")?;
	let (_, chapter_key) = parse_novel_and_chapter(&url)?;
	let chapter_key = chapter_key?;
	let mut title = link.text()?;
	let chapter_number = parse_chapter_number(&title);
	if let Some(chapter_number) = chapter_number {
		title = match title.strip_prefix(&format!("Chapter {chapter_number}")) {
			Some(rest) => rest
				.trim()
				.strip_prefix(':')
				.or_else(|| rest.trim().strip_prefix('-'))
				.map_or_else(|| rest.trim().to_string(), |t| t.trim().to_string()),
			None => title,
		};
	};
	Some(Chapter {
		key: chapter_key,
		title: { if !title.is_empty() { Some(title) } else { None } },
		chapter_number,
		url: Some(url),
		..Default::default()
	})
}

/// Extract Chapters from a single novel chapter-list page, in ascending order.
pub fn extract_chapters(html: &Document) -> Vec<Chapter> {
	let Some(items) = html.select("div.m-newest2 > ul.ul-list5 > li") else {
		return Vec::new();
	};
	items.filter_map(|item| chapter_from_item(&item)).collect()
}

/// Fetch a novel's full chapter list, newest-first (as Aidoku expects).
///
/// `first_page` is the already-loaded novel page, which `build_novel_url` loads
/// at `pageSize=200`, so it is reused for the first page; only the remaining
/// pages are fetched. `request_html` transparently retries on the site's 429
/// rate limit.
pub fn fetch_all_chapters(first_page: &Document, slug: &str) -> Result<Vec<Chapter>> {
	let mut chapters = extract_chapters(first_page);
	let page_count =
		total_chapter_count(first_page).map_or(1, |total| total.div_ceil(CHAPTERS_PER_PAGE));
	for page in 2..=page_count {
		let page_url = format!("{}&page={page}", build_novel_url(slug));
		chapters.extend(extract_chapters(&request_html(&page_url)?));
	}
	// Pages list chapters oldest-first; Aidoku expects newest-first.
	chapters.reverse();
	Ok(chapters)
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
