#![allow(clippy::needless_pass_by_value)]
use crate::BASE_URL;
use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, Result,
	alloc::{String, Vec, collections::BTreeSet, string::ToString},
	imports::{
		html::{Document, Element},
		net::Request,
	},
	prelude::*,
};

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
	if url.is_empty() {
		return None;
	}
	Some(url.replace("ss.jpg", "s.jpg"))
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
	let chapter_key = parts.next().and_then(|seg| {
		if seg.starts_with("chapter-") {
			Some(seg.to_string())
		} else {
			None
		}
	});
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

/// Extract Chapters from Novel page.
pub fn extract_chapters(html: &Document) -> Vec<Chapter> {
	let mut chapters = Vec::new();
	append_chapters_from_doc(html, &mut chapters);
	chapters.reverse();
	chapters
}

fn append_chapters_from_doc(doc: &Document, chapters: &mut Vec<Chapter>) {
	let Some(items) = doc.select("div.m-newest2 > ul.ul-list5 > li") else {
		return;
	};

	for item in items {
		let Some(link) = item.select_first("a[href]") else {
			continue;
		};
		let Some(url) = link.attr("abs:href") else {
			continue;
		};
		let Some((_, Some(chapter_key))) = parse_novel_and_chapter(&url) else {
			continue;
		};
		let mut title = link.text();
		let chapter_number = title.as_deref().and_then(parse_chapter_number);
		title = title.map(|s| {
			if s.starts_with("Chapter ") && s.contains(":") {
				s.split(":").nth(1).unwrap_or(&s).trim().to_string()
			} else {
				s
			}
		});
		chapters.push(Chapter {
			key: chapter_key,
			title,
			chapter_number,
			url: Some(url),
			..Default::default()
		});
	}
}

pub fn extract_chapter_text(html: &Document) -> Result<String> {
	let container_selector = "div.txt";
	let mut parts = Vec::new();

	if let Some(container) = html.select_first(container_selector)
		&& let Some(elms) = container.select("p, h4")
	{
		for part in elms {
			// Remove ADs
			if let Some(thing) = part.select("subtxt") {
				for ad in thing {
					ad.remove();
				}
			}
			if let Some(text) = part.text()
				&& !text.is_empty()
			{
				parts.push(text.to_string());
			}
		}
	}
	if parts.is_empty() {
		bail!("chapter text not found");
	}
	Ok(parts.join("\n\n"))
}

pub fn parse_search_results(html: &Document) -> Vec<Manga> {
	let mut entries = Vec::new();
	let mut seen = BTreeSet::new();
	if let Some(els) = html.select("a[href*='/novel/']") {
		append_anchor_entries(els, &mut entries, &mut seen);
	}
	entries
}

pub fn parse_home_section(html: &Document, heading: &str) -> Vec<Manga> {
	let Some(container) = find_section_container(html, heading) else {
		return Vec::new();
	};
	parse_entries_from_element(&container)
}

fn parse_entries_from_element(root: &Element) -> Vec<Manga> {
	let mut entries = Vec::new();
	let mut seen = BTreeSet::new();
	if let Some(els) = root.select("a[href*='/novel/']") {
		append_anchor_entries(els, &mut entries, &mut seen);
	}
	entries
}

fn find_section_container(html: &Document, heading: &str) -> Option<Element> {
	let selector = format!("div > div h3:contains({heading})");
	html.select_first(&selector)
		.and_then(|h| h.parent())
		.and_then(|p| p.parent())
}

fn append_anchor_entries<I>(anchors: I, entries: &mut Vec<Manga>, seen: &mut BTreeSet<String>)
where
	I: IntoIterator<Item = Element>,
{
	for el in anchors {
		let Some(url) = el.attr("abs:href") else {
			continue;
		};
		let Some((slug, chapter_key)) = parse_novel_and_chapter(&url) else {
			continue;
		};
		if chapter_key.is_some() || seen.contains(&slug) {
			continue;
		}
		let Some(title) = extract_anchor_title(&el) else {
			continue;
		};
		let cover = el.parent().and_then(|p| find_cover_image(&p));
		let manga = Manga {
			key: slug.clone(),
			title,
			cover,
			url: Some(build_novel_url(&slug)),
			..Default::default()
		};
		entries.push(manga);
		seen.insert(slug);
	}
}

fn extract_anchor_title(el: &Element) -> Option<String> {
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
	const DESCRIPTION_QUERY: &str = "h4.abstract + div.txt p";
	if let Some(parts) = html.select(DESCRIPTION_QUERY) {
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
	I: IntoIterator<Item = Element>,
{
	elements
		.into_iter()
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
