#![allow(clippy::needless_pass_by_value)]
use crate::{BASE_URL, USER_AGENT};
use aidoku::{
	Chapter, ContentRating, Manga, Result,
	alloc::{String, Vec, string::ToString},
	imports::{
		html::{Document, Element},
		net::Request,
	},
	prelude::*,
};

pub fn request_html(url: &str) -> Result<Document> {
	Ok(Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.header("Referer", BASE_URL)
		.header("Accept", "text/html,application/xhtml+xml")
		.html()?)
}

pub fn build_novel_url(slug: &str) -> String {
	format!("{BASE_URL}/novel/{slug}")
}

pub fn build_chapter_url(slug: &str, chapter_key: &str) -> String {
	format!("{BASE_URL}/novel/{slug}/{chapter_key}")
}

fn absolute_url(path_or_url: &str) -> String {
	if path_or_url.starts_with("http") {
		path_or_url.into()
	} else if path_or_url.starts_with('/') {
		format!("{BASE_URL}{path_or_url}")
	} else {
		format!("{BASE_URL}/{path_or_url}")
	}
}

pub fn parse_novel_and_chapter(url: &str) -> Option<(String, Option<String>)> {
	let path = url
		.split(['?', '#'])
		.next()
		.unwrap_or(url)
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
	if tags
		.iter()
		.any(|tag| matches!(tag.as_str(), "Adult" | "Mature"))
	{
		ContentRating::NSFW
	} else if tags
		.iter()
		.any(|tag| matches!(tag.as_str(), "Smut" | "Ecchi" | "Yaoi" | "Yuri"))
	{
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	}
}

pub fn extract_title(html: &Document) -> Result<String> {
	meta_content(html, "meta[property='og:title']")
		.or_else(|| meta_content(html, "meta[name='title']"))
		.or_else(|| {
			html.select_first("h1, h2, h3")
				.and_then(|el| el.text())
				.map(|t| t.trim().to_string())
		})
		.filter(|t| !t.is_empty())
		.ok_or_else(|| error!("title not found"))
}

pub fn extract_cover(html: &Document) -> Option<String> {
	meta_content(html, "meta[property='og:image']").or_else(|| {
		html.select_first("img[src*='/files/article/image/']")
			.and_then(|el| el.attr("abs:src"))
	})
}

pub fn extract_description(html: &Document) -> Option<String> {
	meta_content(html, "meta[property='og:description']")
		.or_else(|| meta_content(html, "meta[name='description']"))
		.or_else(|| {
			let container = html.select_first(
				"div:has(h4:contains(SUMMARY)), div:has(h3:contains(SUMMARY)), \
				 div:has(h4:contains(Summary)), div:has(h3:contains(Summary))",
			)?;
			container
				.select("p")
				.map(extract_text_from_elements)
				.filter(|t| !t.is_empty())
		})
}

pub fn extract_authors(html: &Document) -> Option<Vec<String>> {
	let mut authors = Vec::new();
	let elements = html
		.select_first("div:has(a[href^='/author/'])")
		.and_then(|el| el.select("a[href^='/author/']"))
		.or_else(|| html.select("a[href^='/author/']"));
	if let Some(els) = elements {
		for el in els {
			if let Some(text) = el.text() {
				let name = text.trim();
				if !name.is_empty() && !authors.iter().any(|a| a == name) {
					authors.push(name.to_string());
				}
			}
		}
	}
	(!authors.is_empty()).then_some(authors)
}

pub fn extract_tags(html: &Document) -> Option<Vec<String>> {
	let mut tags = extract_meta_tags(html);
	if tags.is_empty() {
		let elements = html
			.select_first("div:has(a[href^='/author/'])")
			.and_then(|el| el.select("a[href^='/genre/']"))
			.or_else(|| html.select("a[href^='/genre/']"));
		if let Some(els) = elements {
			for el in els {
				if let Some(text) = el.text() {
					let tag = text.trim();
					if !tag.is_empty() && !tags.iter().any(|t| t == tag) {
						tags.push(tag.to_string());
						if tags.len() >= 12 {
							break;
						}
					}
				}
			}
		}
	}
	(!tags.is_empty()).then_some(tags)
}

pub fn extract_chapters(html: &Document, slug: &str) -> Vec<Chapter> {
	let mut chapters = Vec::new();
	let mut seen = Vec::new();
	append_chapters_from_doc(html, slug, &mut chapters, &mut seen);
	chapters.reverse();
	chapters
}

fn append_chapters_from_doc(
	doc: &Document,
	slug: &str,
	chapters: &mut Vec<Chapter>,
	seen: &mut Vec<String>,
) {
	if let Some(items) = doc.select("div.m-newest2 > ul.ul-list5 > li") {
		for item in items {
			let Some(link) = item.select_first("a") else {
				continue;
			};
			let Some(href) = link.attr("href") else {
				continue;
			};
			let url = absolute_url(&href);
			let Some((link_slug, Some(chapter_key))) = parse_novel_and_chapter(&url) else {
				continue;
			};
			if link_slug != slug || seen.iter().any(|s| s == &chapter_key) {
				continue;
			}
			let mut title = link
				.text()
				.map(|t| t.trim().to_string())
				.filter(|t| !t.is_empty());
			let chapter_number = title.as_deref().and_then(parse_chapter_number);
			title = title.map(|s| {
				if s.starts_with("Chapter ") {
					if s.contains(":") {
						s.trim_start()
							.split(":")
							.nth(1)
							.unwrap_or(&s)
							.trim()
							.to_string()
					} else {
						"".to_string()
					}
				} else {
					s
				}
			});
			chapters.push(Chapter {
				key: chapter_key.clone(),
				title,
				chapter_number,
				url: Some(url),
				..Default::default()
			});
			seen.push(chapter_key);
		}
	}
}

pub fn extract_chapter_text(html: &Document) -> Result<String> {
	let selectors = [
		"div.txt p",
		"div#chapter-content p",
		"div#chaptercontent p",
		"div.chapter-content p",
		"div#content p",
		"article p",
	];
	for selector in selectors {
		if let Some(els) = html.select(selector) {
			let text = extract_text_from_elements(els);
			if !text.is_empty() {
				return Ok(text);
			}
		}
	}
	let container_selectors = [
		"div.txt",
		"div#chapter-content",
		"div#chaptercontent",
		"div.chapter-content",
		"div#content",
		"article",
	];
	for selector in container_selectors {
		if let Some(el) = html.select_first(selector)
			&& let Some(text) = el.text()
		{
			let cleaned = clean_block_text(&text);
			if !cleaned.is_empty() {
				return Ok(cleaned);
			}
		}
	}
	bail!("chapter text not found")
}

pub fn parse_search_results(html: &Document) -> Vec<Manga> {
	let mut entries = Vec::new();
	let mut seen = Vec::new();
	if let Some(rows) = html.select("div.li-row") {
		append_li_rows(rows, &mut entries, &mut seen);
		if !entries.is_empty() {
			return entries;
		}
	}
	if let Some(els) = html.select("a[href*='/novel/']") {
		append_anchor_entries(els, &mut entries, &mut seen, true);
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
	let mut seen = Vec::new();
	if let Some(rows) = root.select("div.li-row") {
		append_li_rows(rows, &mut entries, &mut seen);
	}
	if entries.is_empty()
		&& let Some(els) = root.select("a[href*='/novel/']")
	{
		append_anchor_entries(els, &mut entries, &mut seen, false);
	}
	entries
}

fn find_section_container(html: &Document, heading: &str) -> Option<Element> {
	let heading_selector = format!(
		"h1:contains({heading}), h2:contains({heading}), h3:contains({heading}), h4:contains({heading}), h5:contains({heading})"
	);
	if let Some(heading_el) = html.select_first(&heading_selector) {
		let mut current = heading_el.parent();
		for _ in 0..6 {
			let Some(el) = current else {
				break;
			};
			if el.select_first("div.li-row, a[href*='/novel/']").is_some() {
				return Some(el);
			}
			current = el.parent();
		}
	}
	let selectors = [
		format!("section:has(h1:contains({heading}))"),
		format!("section:has(h2:contains({heading}))"),
		format!("section:has(h3:contains({heading}))"),
		format!("section:has(h4:contains({heading}))"),
		format!("section:has(h5:contains({heading}))"),
		format!("div:has(h1:contains({heading}))"),
		format!("div:has(h2:contains({heading}))"),
		format!("div:has(h3:contains({heading}))"),
		format!("div:has(h4:contains({heading}))"),
		format!("div:has(h5:contains({heading}))"),
	];
	for selector in &selectors {
		if let Some(el) = html.select_first(selector) {
			return Some(el);
		}
	}
	None
}

fn append_li_rows<I>(rows: I, entries: &mut Vec<Manga>, seen: &mut Vec<String>)
where
	I: IntoIterator<Item = Element>,
{
	for row in rows {
		let url = row
			.select_first("div.pic > a")
			.and_then(|el| el.attr("href"))
			.or_else(|| {
				row.select_first("a[href*='/novel/']")
					.and_then(|el| el.attr("href"))
			});
		let Some(url) = url.map(|u| absolute_url(&u)) else {
			continue;
		};
		let Some((slug, chapter_key)) = parse_novel_and_chapter(&url) else {
			continue;
		};
		if chapter_key.is_some() || seen.iter().any(|s| s == &slug) {
			continue;
		}
		let title = row
			.select_first("div.txt > h3.tit > a")
			.and_then(|el| el.text())
			.or_else(|| {
				row.select_first("div.txt > h3 > a")
					.and_then(|el| el.text())
			})
			.or_else(|| row.select_first("div.txt > h3").and_then(|el| el.text()))
			.and_then(|t| {
				let trimmed = t.trim();
				(!trimmed.is_empty()).then(|| trimmed.to_string())
			});
		let Some(title) = title else {
			continue;
		};
		let cover = find_cover_image(&row);
		let manga = Manga {
			key: slug.clone(),
			title,
			cover,
			url: Some(build_novel_url(&slug)),
			..Default::default()
		};
		entries.push(manga);
		seen.push(slug);
	}
}

fn append_anchor_entries<I>(
	anchors: I,
	entries: &mut Vec<Manga>,
	seen: &mut Vec<String>,
	require_con_parent: bool,
) where
	I: IntoIterator<Item = Element>,
{
	for el in anchors {
		if require_con_parent {
			let parent = match el.parent().and_then(|par| par.parent()) {
				Some(p) => p,
				None => continue,
			};
			if !parent.class_name().unwrap_or_default().eq("con") {
				continue;
			}
		}
		let Some(url) = el.attr("href").map(|u| absolute_url(&u)) else {
			continue;
		};
		let Some((slug, chapter_key)) = parse_novel_and_chapter(&url) else {
			continue;
		};
		if chapter_key.is_some() || seen.iter().any(|s| s == &slug) {
			continue;
		}
		let title = extract_anchor_title(&el)
			.or_else(|| el.parent().and_then(|p| extract_anchor_title(&p)));
		let Some(title) = title else {
			continue;
		};
		let cover =
			find_cover_image(&el).or_else(|| el.parent().and_then(|p| find_cover_image(&p)));
		let manga = Manga {
			key: slug.clone(),
			title,
			cover,
			url: Some(build_novel_url(&slug)),
			..Default::default()
		};
		entries.push(manga);
		seen.push(slug);
	}
}

fn extract_anchor_title(el: &Element) -> Option<String> {
	let title = el
		.text()
		.or_else(|| el.attr("title"))
		.or_else(|| el.select_first("img").and_then(|img| img.attr("alt")))
		.or_else(|| el.select_first("h3, h4, h5").and_then(|h| h.text()));
	title.and_then(|t| {
		let trimmed = t.trim();
		(!trimmed.is_empty()).then(|| trimmed.to_string())
	})
}

fn meta_content(html: &Document, selector: &str) -> Option<String> {
	html.select_first(selector)
		.and_then(|el| el.attr("content"))
}

fn extract_meta_tags(html: &Document) -> Vec<String> {
	let mut tags = Vec::new();
	if let Some(els) = html.select("meta[property='article:tag']") {
		for el in els {
			if let Some(content) = el.attr("content") {
				let tag = content.trim();
				if !tag.is_empty() && !tags.iter().any(|t| t == tag) {
					tags.push(tag.to_string());
				}
			}
		}
	}
	tags
}

fn extract_text_from_elements<I>(elements: I) -> String
where
	I: IntoIterator<Item = Element>,
{
	let mut parts = Vec::new();
	for el in elements {
		if let Some(text) = el.text() {
			let trimmed = text.trim();
			if !trimmed.is_empty() {
				parts.push(trimmed.to_string());
			}
		}
	}
	parts.join("\n\n")
}

fn clean_block_text(text: &str) -> String {
	let mut parts = Vec::new();
	for line in text.lines() {
		let trimmed = line.trim();
		if trimmed.is_empty() {
			continue;
		}
		let lower = trimmed.to_ascii_lowercase();
		if lower.starts_with("prev chapter")
			|| lower.starts_with("next chapter")
			|| lower.starts_with("add to library")
			|| lower.contains("freewebnovel.com")
			|| lower.contains("read books online")
		{
			continue;
		}
		parts.push(trimmed.to_string());
	}
	parts.join("\n\n")
}

fn find_cover_image(el: &Element) -> Option<String> {
	let cover = el
		.select_first("div.pic > a > img")
		.and_then(|img| img.attr("src"))
		.or_else(|| el.select_first("img").and_then(|img| img.attr("src")));
	cover.and_then(|url| {
		let trimmed = url.trim();
		(!trimmed.is_empty()).then(|| absolute_url(trimmed))
	})
}
