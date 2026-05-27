#![allow(clippy::needless_pass_by_value)]
use aidoku::{
	Chapter, ContentRating, Manga, Result,
	alloc::{String, Vec, string::ToString},
	imports::{html::{Document, Element}, net::Request},
	prelude::*,
};
use core::cmp::Ordering;

use crate::{BASE_URL, USER_AGENT};

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
	let mut num = String::new();
	let mut seen_dot = false;
	for ch in name.chars() {
		if ch.is_ascii_digit() {
			num.push(ch);
		} else if ch == '.' && !seen_dot && !num.is_empty() {
			seen_dot = true;
			num.push(ch);
		} else if !num.is_empty() {
			break;
		}
	}
	num.parse().ok()
}

pub fn content_rating_from_tags(tags: &[String]) -> ContentRating {
	if tags.iter().any(|tag| matches!(tag.as_str(), "Adult" | "Mature")) {
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

pub fn extract_title(html: &Document) -> Option<String> {
	meta_content(html, "meta[property='og:title']")
		.or_else(|| meta_content(html, "meta[name='title']"))
		.or_else(|| {
			html.select_first("h1, h2, h3")
				.and_then(|el| el.text())
				.map(|t| t.trim().to_string())
		})
		.filter(|t| !t.is_empty())
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
	if let Some(els) = html.select("a[href*='/novel/']") {
		for el in els {
			let url = match el.attr("abs:href") {
				Some(u) => u,
				None => continue,
			};
			let Some((link_slug, Some(chapter_key))) = parse_novel_and_chapter(&url) else {
				continue;
			};
			if link_slug != slug || seen.iter().any(|s| s == &chapter_key) {
				continue;
			}
			let title = el
				.text()
				.map(|t| t.trim().to_string())
				.filter(|t| !t.is_empty());
			let chapter_number = title
				.as_deref()
				.and_then(parse_chapter_number)
				.or_else(|| parse_chapter_number(&chapter_key));
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
	if chapters.len() > 1 {
		chapters.sort_by(|a, b| match (a.chapter_number, b.chapter_number) {
			(Some(left), Some(right)) => left
				.partial_cmp(&right)
				.unwrap_or(Ordering::Equal),
			(Some(_), None) => Ordering::Less,
			(None, Some(_)) => Ordering::Greater,
			(None, None) => Ordering::Equal,
		});
	}
	chapters
}

pub fn extract_chapter_text(html: &Document) -> String {
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
				return text;
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
		if let Some(el) = html.select_first(selector) {
			if let Some(text) = el.text() {
				let cleaned = clean_block_text(&text);
				if !cleaned.is_empty() {
					return cleaned;
				}
			}
		}
	}
	String::new()
}

pub fn parse_search_results(html: &Document) -> Vec<Manga> {
	let mut entries = Vec::new();
	let mut seen = Vec::new();
	if let Some(rows) = html.select("div.li-row") {
		for row in rows {
			let url = row
				.select_first("div.pic > a")
				.and_then(|el| el.attr("href"))
				.or_else(|| row.select_first("a[href*='/novel/']").and_then(|el| el.attr("href")));
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
				.or_else(|| row.select_first("div.txt > h3 > a").and_then(|el| el.text()))
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
		if !entries.is_empty() {
			return entries;
		}
	}
	if let Some(els) = html.select("a[href*='/novel/']") {
		for el in els {
			let parent = match el.parent().and_then(|par| par.parent()) {
				Some(p) => p,
				None => continue,
			};
			if !parent.class_name().unwrap_or_default().eq("con") {
				continue;
			}
			let Some(url) = parent
				.select_first("a")
				.and_then(|a| a.attr("href"))
				.map(|u| absolute_url(&u))
			else {
				continue;
			};
			let Some((slug, chapter_key)) = parse_novel_and_chapter(&url) else {
				continue;
			};
			if chapter_key.is_some() || seen.iter().any(|s| s == &slug) {
				continue;
			}
			let title = match el.text() {
				Some(t) => {
					let trimmed = t.trim();
					if trimmed.is_empty() {
						continue;
					}
					trimmed.to_string()
				}
				None => continue,
			};
			let cover = find_cover_image(&el);
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
	entries
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
