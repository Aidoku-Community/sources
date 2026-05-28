use aidoku::imports::html::Document;
use aidoku::{Chapter, ContentRating, Manga, MangaStatus, Page, PageContent, Viewer};
use alloc::{
	string::{String, ToString},
	vec::Vec,
};

use crate::helpers::clean_title;

pub fn parse_listing(doc: &Document, lang_filters: &[String]) -> (Vec<Manga>, bool) {
	let mut entries: Vec<Manga> = Vec::new();
	let articles =
		match doc.select("article.post, div.post, article.item, div.item, ul.wpp-list li") {
			Some(a) => a,
			None => return (entries, false),
		};

	for article in articles {
		if let Some(class_attr) = article.attr("class") {
			if class_attr.contains("category-video") {
				continue;
			}

			if !lang_filters.is_empty() {
				let has_lang = lang_filters
					.iter()
					.any(|lang| class_attr.contains(&alloc::format!("lang-{}", lang)));
				if !has_lang {
					continue;
				}
			}
		}

		let link = article
			.select_first(".entry-title a")
			.or_else(|| article.select_first("h1 a, h2 a, h3 a"))
			.or_else(|| article.select_first("a.wpp-post-title"));

		let (key, title_raw) = match link {
			Some(a) => {
				let href = a.attr("abs:href").unwrap_or_default();
				let text = a.text().unwrap_or_default();
				if href.is_empty() {
					continue;
				}
				(href, text)
			}
			None => continue,
		};

		if entries.iter().any(|e| e.key == key) {
			continue;
		}

		let title = clean_title(&title_raw);
		let cover = article
			.select_first("img.post-image")
			.or_else(|| article.select_first("img.entry-image"))
			.or_else(|| article.select_first("img.wpp-thumbnail"))
			.or_else(|| article.select_first("img"))
			.and_then(|img| img.attr("abs:src"))
			.map(strip_thumbnail_size);

		entries.push(Manga {
			key,
			title,
			cover,
			..Default::default()
		});
	}

	let has_next = doc
		.select_first("a.next.page-numbers, li.pagination-next a")
		.is_some();

	(entries, has_next)
}

/// Strip WP thumbnail size suffixes
fn strip_thumbnail_size(src: String) -> String {
	if let Some(dash) = src.rfind('-') {
		let suffix = &src[dash + 1..];
		if let Some((dims, ext)) = suffix.split_once('.') {
			if dims.contains('x') && dims.chars().all(|c| c.is_ascii_digit() || c == 'x') {
				return alloc::format!("{}.{}", &src[..dash], ext);
			}
		}
	}
	src
}

fn lang_display_to_code(name: &str) -> &str {
	match name.to_lowercase().trim() {
		"english" => "en",
		"japanese" => "ja",
		"chinese" => "zh",
		"korean" => "ko",
		"spanish" => "es",
		"french" => "fr",
		"german" => "de",
		"italian" => "it",
		"portuguese" => "pt",
		_ => name,
	}
}

pub fn parse_manga(doc: &Document, key: &str) -> aidoku::imports::error::Result<Manga> {
	let title_raw = doc
		.select_first("h1.entry-title")
		.and_then(|e| e.text())
		.unwrap_or_default();
	let title = clean_title(&title_raw);

	let cover = doc
		.select_first("script[type='application/ld+json']")
		.and_then(|s| s.text())
		.and_then(|json| {
			// Extract "thumbnailUrl":"..." without a JSON parser.
			let key = "\"thumbnailUrl\":\"";
			let start = json.find(key)? + key.len();
			let end = json[start..].find('"')? + start;
			Some(json[start..end].replace("\\/", "/"))
		});

	let mut tags: Vec<String> = Vec::new();
	let mut authors: Vec<String> = Vec::new();
	let mut chapter_language: Option<String> = None;

	if let Some(meta_spans) = doc.select(
		"p.entry-meta span.entry-terms, \
		 p.entry-meta span.entry-tags, \
		 p.entry-meta span.entry-categories",
	) {
		// The meta block appears in both the head and footer according to the html
		let mut seen_creator = false;
		let mut seen_lang = false;

		for span in meta_spans {
			let label = span
				.select_first(".meta-label")
				.and_then(|l| l.text())
				.unwrap_or_default();
			let class_attr = span.attr("class").unwrap_or_default();

			let links: Vec<String> = span
				.select("a")
				.into_iter()
				.flatten()
				.filter_map(|a| a.text())
				.map(|t| t.trim().to_string())
				.filter(|t| !t.is_empty())
				.collect();

			if label.contains("Creator") {
				if !seen_creator {
					authors.extend(links);
					seen_creator = true;
				}
			} else if label.contains("Lang") {
				if !seen_lang {
					if let Some(lang_name) = links.first() {
						chapter_language = Some(lang_display_to_code(lang_name).to_string());
					}
					seen_lang = true;
				}
			} else if label.contains("Genre")
				|| class_attr.contains("entry-tags")
				|| class_attr.contains("entry-categories")
			{
				for link in links {
					if !tags.contains(&link) {
						tags.push(link);
					}
				}
			}
		}
	}

	let mut chapters: Vec<Chapter> = Vec::new();

	if let Some(links) = doc.select("div.entry-pagination a.page-numbers:not(.next):not(.prev)") {
		let page_links: Vec<_> = links.collect();
		if !page_links.is_empty() {
			chapters.push(Chapter {
				key: key.to_string(),
				chapter_number: Some(1.0),
				language: chapter_language.clone(),
				url: Some(key.to_string()),
				..Default::default()
			});
			for link in &page_links {
				let href = link.attr("abs:href").unwrap_or_default();
				let num: f32 = link
					.text()
					.as_deref()
					.unwrap_or("")
					.trim()
					.parse()
					.unwrap_or(0.0);
				if num > 1.0 && !chapters.iter().any(|c| c.chapter_number == Some(num)) {
					chapters.push(Chapter {
						key: href.clone(),
						chapter_number: Some(num),
						language: chapter_language.clone(),
						url: Some(href),
						..Default::default()
					});
				}
			}
		}
	}

	if chapters.is_empty() {
		chapters.push(Chapter {
			key: key.to_string(),
			chapter_number: Some(1.0),
			language: chapter_language,
			url: Some(key.to_string()),
			..Default::default()
		});
	} else {
		// I'm not sure about this but this looks cleaner when I tested
		chapters.reverse();
	}

	Ok(Manga {
		key: key.to_string(),
		title,
		cover,
		authors: if authors.is_empty() {
			None
		} else {
			Some(authors.clone())
		},
		artists: if authors.is_empty() {
			None
		} else {
			Some(authors)
		},
		url: Some(key.to_string()),
		tags: if tags.is_empty() { None } else { Some(tags) },
		status: MangaStatus::Completed,
		content_rating: ContentRating::NSFW,
		viewer: Viewer::RightToLeft,
		chapters: Some(chapters),
		..Default::default()
	})
}

pub fn parse_pages(doc: &Document) -> Vec<Page> {
	let mut pages: Vec<Page> = Vec::new();

	if let Some(imgs) = doc.select("img.img-myreadingmanga") {
		for img in imgs {
			let src = img.attr("abs:src").unwrap_or_default();
			if !src.is_empty() {
				pages.push(Page {
					content: PageContent::url(src),
					..Default::default()
				});
			}
		}
	}

	pages
}
