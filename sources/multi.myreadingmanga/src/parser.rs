use aidoku::imports::html::Document;
use aidoku::{Chapter, ContentRating, Manga, MangaStatus, Page, PageContent, Viewer, prelude::*};
use alloc::{
	string::{String, ToString},
	vec::Vec,
};

use crate::helpers::{BASE_URL, HTTP_URL, clean_title};

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
					.any(|lang| class_attr.contains(&format!("lang-{}", lang)));
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
				let href = a.attr("href").unwrap_or_default();
				let text = a.text().unwrap_or_default();
				if href.is_empty() {
					continue;
				}
				let key = href
					.trim_start_matches(BASE_URL)
					.trim_start_matches(HTTP_URL)
					.trim_end_matches('/')
					.to_string();
				if key.is_empty() {
					continue;
				}
				(key, text)
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
		if let Some((dims, ext)) = suffix.split_once('.')
			&& dims.contains('x')
			&& dims.chars().all(|c| c.is_ascii_digit() || c == 'x')
		{
			return format!("{}.{}", &src[..dash], ext);
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

pub fn parse_manga(doc: &Document, manga: &mut Manga) {
	if let Some(title) = doc
		.select_first("h1.entry-title")
		.and_then(|e| e.text())
		.map(|t| clean_title(&t))
		&& !title.is_empty()
	{
		manga.title = title;
	}

	// search results cover is more reliable than the first page of the entry.
	let has_cover = manga.cover.as_ref().is_some_and(|c| !c.is_empty());

	if !has_cover {
		let mut fallback_cover = None;

		// saerch results have a yoast schema with the thumbnail we need.
		if let Some(schema) = doc.select_first("script.yoast-schema-graph")
			&& let Some(json) = schema.text()
		{
			let key = "\"thumbnailUrl\":\"";
			if let Some(start_idx) = json.find(key) {
				let start = start_idx + key.len();
				if let Some(end_idx) = json[start..].find('"') {
					fallback_cover = Some(json[start..start + end_idx].replace("\\/", "/"));
				}
			}
		}

		if fallback_cover.is_none() {
			fallback_cover = doc
				.select_first("img.img-myreadingmanga")
				.and_then(|i| i.attr("abs:src"));
		}

		if let Some(cover) = fallback_cover
			&& !cover.is_empty()
		{
			manga.cover = Some(cover);
		}
	}

	let mut tags: Vec<String> = Vec::new();
	let mut authors: Vec<String> = Vec::new();
	let mut seen_creator = false;
	let mut seen_lang = false;

	if let Some(meta_spans) = doc.select(
		"p.entry-meta span.entry-terms, \
p.entry-meta span.entry-tags, \
p.entry-meta span.entry-categories",
	) {
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

			if label.contains("Creator") && !seen_creator {
				authors.extend(links);
				seen_creator = true;
			} else if label.contains("Lang") && !seen_lang {
				seen_lang = true;
				// language is only needed for chapters, skip here
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

	manga.authors = if authors.is_empty() {
		None
	} else {
		Some(authors.clone())
	};
	manga.artists = if authors.is_empty() {
		None
	} else {
		Some(authors)
	};
	manga.tags = if tags.is_empty() { None } else { Some(tags) };
	manga.status = MangaStatus::Completed; // i don't think MRM exposes this on the entry page outside of the search query page.
	manga.content_rating = ContentRating::NSFW;
	manga.viewer = Viewer::RightToLeft; // there's a western comic warning. once i get to know which element controls it i will tackle it.
	manga.url = Some(alloc::format!("{}/{}/", BASE_URL, manga.key));
}

pub fn parse_chapters(doc: &Document, key: &str) -> Vec<Chapter> {
	let chapter_language: Option<String> = doc
		.select("p.entry-meta span.entry-terms")
		.and_then(|mut spans| {
			spans.find(|span| {
				span.select_first(".meta-label")
					.and_then(|l| l.text())
					.is_some_and(|t| t.contains("Lang"))
			})
		})
		.and_then(|span| span.select_first("a"))
		.and_then(|a| a.text())
		.map(|t| lang_display_to_code(t.trim()).to_string());

	let mut chapters: Vec<Chapter> = Vec::new();

	if let Some(links) = doc.select("div.entry-pagination a.page-numbers:not(.next):not(.prev)") {
		let page_links: Vec<_> = links.collect();
		if !page_links.is_empty() {
			// Page 1 is the post itself (shown as a <span>, not a link)
			chapters.push(Chapter {
				key: key.to_string(),
				chapter_number: Some(1.0),
				language: chapter_language.clone(),
				url: Some(alloc::format!("{}/{}/", BASE_URL, key)),
				..Default::default()
			});

			for link in page_links.iter().rev() {
				let href = link.attr("href").unwrap_or_default();
				let num: f32 = link
					.text()
					.as_deref()
					.unwrap_or("")
					.trim()
					.parse()
					.unwrap_or(0.0);

				let chapter_key = href
					.trim_start_matches(BASE_URL)
					.trim_start_matches(HTTP_URL)
					.trim_end_matches('/')
					.to_string();

				if num > 1.0
					&& !chapter_key.is_empty()
					&& !chapters.iter().any(|c| c.chapter_number == Some(num))
				{
					chapters.push(Chapter {
						key: chapter_key,
						chapter_number: Some(num),
						language: chapter_language.clone(),
						url: Some(href),
						..Default::default()
					});
				}
			}

			chapters.sort_by(|a, b| {
				b.chapter_number
					.partial_cmp(&a.chapter_number)
					.unwrap_or(core::cmp::Ordering::Equal)
			});
		}
	}

	if chapters.is_empty() {
		chapters.push(Chapter {
			key: key.to_string(),
			chapter_number: Some(1.0),
			language: chapter_language,
			url: Some(alloc::format!("{}/{}/", BASE_URL, key)),
			..Default::default()
		});
	}

	chapters
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
