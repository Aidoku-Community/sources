#![no_std]

use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeLayout,
	Listing, ListingProvider, Manga, MangaPageResult, MangaStatus, Page, PageContent, Result,
	Source, Viewer,
	alloc::{String, Vec, format},
	imports::{net::Request, std::send_partial_result},
	prelude::*,
};

struct Hennoveltranslations;

const BASE_URL: &str = "https://hennoveltranslations.com";

fn parse_chapter_number(title: &str) -> Option<f32> {
	let words: Vec<&str> = title.split_whitespace().collect();
	if let Some(last) = words.last() {
		return last.parse::<f32>().ok();
	}
	None
}

fn extract_meta_value(text: &str, label: &str) -> String {
	if let Some(pos) = text.find(label) {
		let after = &text[pos + label.len()..];
		let end = after.find(['\n', '.', '•', ':']).unwrap_or(after.len());
		String::from(after[..end].trim())
	} else {
		String::new()
	}
}

fn content_rating_from_tags(tags: &[String]) -> ContentRating {
	const NSFW_TAGS: &[&str] = &["Adult", "Mature", "Smut"];
	if tags.iter().any(|tag| NSFW_TAGS.contains(&tag.as_str())) {
		ContentRating::NSFW
	} else {
		ContentRating::Safe
	}
}

impl Source for Hennoveltranslations {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		_query: Option<String>,
		_page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let url = format!("{}/archives/novels", BASE_URL);
		println!("Fetching novel list: {}", url);
		let html = Request::get(&url)?.html()?;
		let mut entries = Vec::new();

		if let Some(articles) = html.select("article.novels") {
			for article in articles {
				if let Some(link) = article.select_first(".entry-title a") {
					let title = link.text().unwrap_or_default();
					if let Some(href) = link.attr("href") {
						let key = String::from(
							href.replace(&format!("{}/archives/novels/", BASE_URL), "")
								.trim_end_matches('/'),
						);

						let cover = article
							.select_first(".post-image img")
							.and_then(|img| img.attr("src"));

						println!("  Novel: {} -> {}", title, key);

						entries.push(Manga {
							key,
							title,
							cover,
							url: Some(href),
							..Default::default()
						});
					}
				}
			}
		}

		Ok(MangaPageResult {
			entries,
			has_next_page: false,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{}/archives/novels/{}", BASE_URL, manga.key);
		let html = Request::get(&url)?.html()?;

		if needs_details {
			manga.title = html
				.select("h1")
				.and_then(|el| el.text())
				.unwrap_or_default();
			manga.description = html
				.select(".novel-content, .entry-content")
				.and_then(|el| el.text());
			manga.url = Some(url);

			manga.cover = html
				.select_first(".novel-content img, .wp-post-image")
				.and_then(|img| img.attr("src"));

			let meta_text = html
				.select(".custom-fields, .novel-content")
				.and_then(|el| el.text())
				.unwrap_or_default();

			manga.status = match extract_meta_value(&meta_text, "Status:").as_str() {
				"Completed" => MangaStatus::Completed,
				"Ongoing" => MangaStatus::Ongoing,
				_ => MangaStatus::Unknown,
			};

			let author = extract_meta_value(&meta_text, "Author:");
			if !author.is_empty() {
				manga.authors = Some(Vec::from([author]));
			}

			let genre_str = extract_meta_value(&meta_text, "Genre:");
			if !genre_str.is_empty() {
				let tags: Vec<String> = genre_str
					.split(',')
					.map(|s| String::from(s.trim()))
					.collect();
				manga.content_rating = content_rating_from_tags(&tags);
				manga.tags = Some(tags);
			} else {
				manga.content_rating = ContentRating::Unknown;
			}

			let type_str = extract_meta_value(&meta_text, "Type:");
			if type_str.to_lowercase().contains("manhwa") {
				manga.viewer = Viewer::Webtoon;
			}

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let mut chapters = Vec::new();

			if let Some(free_list) = html.select(".episode-list2")
				&& let Some(links) = free_list.select("a")
			{
				for node in links {
					if let Some(chapter_url) = node.attr("href")
						&& chapter_url.contains("/archives/episodes/")
					{
						let title = node.text().unwrap_or_default();
						let key = chapter_url.replace(BASE_URL, "");

						chapters.push(Chapter {
							key,
							title: Some(String::from(&title)),
							chapter_number: parse_chapter_number(&title),
							url: Some(chapter_url),
							..Default::default()
						});
					}
				}
			}

			println!("  Chapters found: {}", chapters.len());
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{}{}", BASE_URL, chapter.key);
		println!("  Loading page: {}", url);
		let html = Request::get(&url)?.html()?;

		let is_paywalled = html
			.select(".patreon-locked-content-message")
			.is_some_and(|el| !el.is_empty());

		println!("  Paywalled: {}", is_paywalled);

		let subheading = html
			.select(".episode-content h2")
			.and_then(|el| el.text())
			.or_else(|| html.select(".episode-content h1").and_then(|el| el.text()))
			.unwrap_or_default();

		let mut paragraphs = Vec::new();

		if !subheading.is_empty() {
			paragraphs.push(format!("## {}", subheading));
		}

		if let Some(content) = html.select(".episode-content")
			&& let Some(elements) = content.select("p")
		{
			for p in elements {
				let text = p.text().unwrap_or_default();
				if !text.is_empty() {
					paragraphs.push(text);
				}
			}
		}

		if paragraphs.len() <= 1
			&& !subheading.is_empty()
			&& let Some(content) = html.select(".entry-content, .reading-content")
			&& let Some(elements) = content.select("p")
		{
			for p in elements {
				let text = p.text().unwrap_or_default();
				if !text.is_empty() {
					paragraphs.push(text);
				}
			}
		}

		if paragraphs.len() <= 1 {
			let fallback = html
				.select(".episode-content, .entry-content, .reading-content")
				.and_then(|el| el.text())
				.unwrap_or_default();
			if !fallback.is_empty() {
				paragraphs.push(fallback);
			}
		}

		if is_paywalled {
			paragraphs.push(String::from(
				"This chapter is locked behind a paywall and will be released for free at a later date.",
			));
		}

		let text_content = paragraphs.join("\n\n");

		Ok(Vec::from([Page {
			content: PageContent::Text(text_content),
			..Default::default()
		}]))
	}
}

impl ListingProvider for Hennoveltranslations {
	fn get_manga_list(&self, _listing: Listing, _page: i32) -> Result<MangaPageResult> {
		self.get_search_manga_list(None, 1, Vec::new())
	}
}

impl Home for Hennoveltranslations {
	fn get_home(&self) -> Result<HomeLayout> {
		bail!("Home page not implemented")
	}
}

impl DeepLinkHandler for Hennoveltranslations {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url
			.split(['?', '#'])
			.next()
			.unwrap_or(&url)
			.strip_prefix(&format!("{}/", BASE_URL))
			.unwrap_or("");

		if let Some(slug) = path.strip_prefix("archives/novels/")
			&& !slug.is_empty()
		{
			let key = String::from(slug.trim_end_matches('/'));
			return Ok(Some(DeepLinkResult::Manga { key }));
		}

		if let Some(episode_path) = path.strip_prefix("archives/episodes/")
			&& !episode_path.is_empty()
		{
			let key = String::from(episode_path.trim_end_matches('/'));
			let chapter_key = format!("/archives/episodes/{}", key);
			let manga_key = String::new();
			return Ok(Some(DeepLinkResult::Chapter {
				manga_key,
				key: chapter_key,
			}));
		}

		Ok(None)
	}
}

register_source!(Hennoveltranslations, ListingProvider, Home, DeepLinkHandler);
