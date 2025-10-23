use crate::{BASE_URL, net::Url};
use aidoku::{
	Manga, MangaPageResult, MangaStatus, Page, Result, Viewer,
	alloc::{String, Vec, string::ToString as _, vec},
	error,
	imports::{
		html::{Document, Element, ElementList},
		net::Request,
	},
	prelude::*,
};
use regex::Regex;

fn extract_chapter_number(title: &str) -> Option<f32> {
	// This handles cases like "第183话 180" where 180 is the actual chapter
	let re1 =
		Regex::new(r"(?:第\s*\d+(?:\.\d+)?\s*(?:话|話|章|回|卷|册|冊)\s*)(\d+(?:\.\d+)?)").ok()?;
	if let Some(captures) = re1.captures(title)
		&& let Some(num_match) = captures.get(1)
		&& let Ok(num) = num_match.as_str().parse::<f32>()
	{
		return Some(num);
	}

	// Second try: match "第X话" pattern where X is the chapter number
	let re2 = Regex::new(r"(?:第\s*)(\d+(?:\.\d+)?)\s*(?:话|話|章|回|卷|册|冊)").ok()?;
	if let Some(captures) = re2.captures(title)
		&& let Some(num_match) = captures.get(1)
		&& let Ok(num) = num_match.as_str().parse::<f32>()
	{
		return Some(num);
	}

	// Third try: match pure number at the beginning
	let re3 = Regex::new(r"^(\d+(?:\.\d+)?)").ok()?;
	if let Some(captures) = re3.captures(title)
		&& let Some(num_match) = captures.get(1)
		&& let Ok(num) = num_match.as_str().parse::<f32>()
	{
		return Some(num);
	}

	None
}

pub trait MangaPage {
	fn update_details(&self, manga: &mut Manga) -> Result<()>;
	fn manga_page_result(&self) -> Result<MangaPageResult>;
}

impl MangaPage for Document {
	fn update_details(&self, manga: &mut Manga) -> Result<()> {
		// Extract cover from meta tag or resize URL
		manga.cover = self
			.select_first("meta[name='og:image']")
			.and_then(|meta| meta.attr("content"))
			.or_else(|| {
				self.select_first("amp-img.comic-cover")
					.and_then(|img| img.attr("src"))
			});

		// Remove query params from cover if exists
		if let Some(ref cover) = manga.cover
			&& let Some(pos) = cover.rfind('?')
		{
			manga.cover = Some(cover[..pos].to_string());
		}

		manga.title = self
			.select_first("meta[name='og:novel:book_name']")
			.and_then(|meta| meta.attr("content"))
			.unwrap_or_default();

		let author = self
			.select_first("meta[name='og:novel:author']")
			.and_then(|meta| meta.attr("content"))
			.unwrap_or_default();

		// Deduplicate and join artists
		let mut artists: Vec<String> = author
			.split(',')
			.map(|s| s.trim().to_string())
			.filter(|s| !s.is_empty())
			.collect();
		artists.dedup();
		let artist_str = artists.join(", ");

		manga.authors = Some(vec![artist_str]);

		// Extract description
		manga.description = self
			.select_first("meta[name='og:description']")
			.and_then(|meta| meta.attr("content"))
			.map(|desc| {
				// Remove prefix if exists
				if let Some(pos) = desc.find("》全集，") {
					desc[pos + "》全集，".len()..].trim().to_string()
				} else {
					desc
				}
			});

		// Extract categories/tags
		let tags = self
			.try_select("span.tag")?
			.skip(1) // Skip first tag (usually status)
			.filter_map(|tag| tag.text())
			.filter(|t| !t.is_empty())
			.collect::<Vec<String>>();
		manga.tags = Some(tags);

		let tags = manga.tags.as_deref().unwrap_or(&[]);
		manga.viewer = if tags
			.iter()
			.any(|tag| tag.contains("國漫") || tag.contains("韓國"))
		{
			Viewer::Webtoon
		} else if tags.iter().any(|tag| tag.contains("日本")) {
			Viewer::RightToLeft
		} else {
			Viewer::LeftToRight
		};

		// Extract status
		let status_str = self
			.select_first("meta[name='og:novel:status']")
			.and_then(|meta| meta.attr("content"))
			.unwrap_or_default();
		manga.status = match status_str.as_str() {
			"連載中" | "连载中" => MangaStatus::Ongoing,
			"已完結" | "已完结" => MangaStatus::Completed,
			_ => MangaStatus::Unknown,
		};

		manga.url = Some(Url::manga(manga.key.clone()).to_string());
		Ok(())
	}

	fn manga_page_result(&self) -> Result<MangaPageResult> {
		let mut entries: Vec<Manga> = Vec::new();

		for item in self.try_select("div.comics-card")? {
			let url = item
				.select_first("a.comics-card__poster")
				.and_then(|a| a.attr("href"))
				.unwrap_or_default();

			let Some(key) = url
				.split('/')
				.filter(|s| !s.is_empty())
				.next_back()
				.map(|s| s.to_string())
			else {
				continue;
			};

			let cover = item
				.select_first("amp-img[noloading]")
				.and_then(|img| img.attr("src"))
				.map(|src| {
					// Remove query params
					if let Some(pos) = src.rfind('?') {
						src[..pos].to_string()
					} else {
						src
					}
				});

			let title = item
				.select_first("h3")
				.and_then(|h3| h3.text())
				.unwrap_or_default();

			let artist = item
				.select_first("small")
				.and_then(|small| small.text())
				.map(|text| {
					let mut artists: Vec<String> = text
						.split(',')
						.map(|s| s.trim().to_string())
						.filter(|s| !s.is_empty())
						.collect();
					artists.dedup();
					artists.join(", ")
				})
				.unwrap_or_default();

			let tags = item
				.select("span")
				.map(|spans| {
					spans
						.filter_map(|span| span.text())
						.filter(|t| !t.is_empty())
						.collect::<Vec<String>>()
				})
				.unwrap_or_default();

			entries.push(Manga {
				key,
				cover,
				title,
				authors: Some(vec![artist]),
				url: Some(format!("{}{}", BASE_URL, url)),
				tags: Some(tags),
				..Default::default()
			});
		}

		Ok(MangaPageResult {
			entries,
			has_next_page: false,
		})
	}
}

pub trait ChapterPage {
	fn chapters(&self, manga_id: &str) -> Result<Vec<aidoku::Chapter>>;
}

impl ChapterPage for Document {
	fn chapters(&self, _manga_id: &str) -> Result<Vec<aidoku::Chapter>> {
		let mut chapters: Vec<aidoku::Chapter> = Vec::new();

		let chapter_items = self.try_select("div.pure-g[id] a.comics-chapters__item")?;

		let mut index = 0.0;
		for item in chapter_items {
			index += 1.0;
			let url = item.attr("href").unwrap_or_default();

			// Extract chapter_id from URL
			let key = url.split('=').next_back().unwrap_or("").to_string();

			let title = item.text().unwrap_or_default();

			let chapter_or_volume = extract_chapter_number(&title).unwrap_or(index);
			let (ch, vo) = if title.trim().ends_with('卷') {
				(-1.0, chapter_or_volume)
			} else {
				(chapter_or_volume, -1.0)
			};

			let scanlator = if vo > -1.0 {
				"单行本".to_string()
			} else {
				"默认".to_string()
			};

			chapters.push(aidoku::Chapter {
				key,
				title: Some(title),
				volume_number: (vo >= 0.0).then_some(vo),
				chapter_number: (ch >= 0.0).then_some(ch),
				url: Some(format!("{}{}", BASE_URL, url)),
				scanlators: Some(vec![scanlator]),
				..Default::default()
			});
		}

		chapters.reverse();
		Ok(chapters)
	}
}

pub trait PageList {
	fn pages(&self) -> Result<Vec<Page>>;
}

impl PageList for Document {
	fn pages(&self) -> Result<Vec<Page>> {
		let mut pages: Vec<Page> = Vec::new();

		for item in self.try_select("amp-img.comic-contain__item")? {
			let url = item.attr("data-src").unwrap_or_default();

			if !url.is_empty() {
				pages.push(Page {
					content: aidoku::PageContent::url(url),
					..Default::default()
				});
			}
		}

		// Handle pagination - check for next chapter
		if let Some(next_link) = self.select_first("a#next-chapter:has(i.icon-xiangxia)")
			&& let Some(next_url) = next_link.attr("href")
			&& !next_url.is_empty()
			&& next_url != "#"
		{
			// Fetch next page
			let next_html = if next_url.starts_with("http") {
				Request::get(next_url)?
			} else {
				Request::get(format!("{}{}", BASE_URL, next_url))?
			}
			.header("Referer", BASE_URL)
			.html()?;

			for item in next_html.try_select("amp-img.comic-contain__item")? {
				let url = item.attr("data-src").unwrap_or_default();

				if !url.is_empty() {
					pages.push(Page {
						content: aidoku::PageContent::url(url),
						..Default::default()
					});
				}
			}
		}

		// Deduplicate pages by URL
		let mut unique_pages: Vec<Page> = Vec::new();
		let mut seen_urls: Vec<String> = Vec::new();

		for page in pages {
			if let aidoku::PageContent::Url(ref url, _) = page.content
				&& !seen_urls.contains(url)
			{
				seen_urls.push(url.clone());
				unique_pages.push(page);
			}
		}

		Ok(unique_pages)
	}
}

trait TrySelect {
	fn try_select<S: AsRef<str>>(&self, css_query: S) -> Result<ElementList>;
}

impl TrySelect for Document {
	fn try_select<S: AsRef<str>>(&self, css_query: S) -> Result<ElementList> {
		self.select(&css_query)
			.ok_or_else(|| error!("No element found for selector: `{}`", css_query.as_ref()))
	}
}

impl TrySelect for Element {
	fn try_select<S: AsRef<str>>(&self, css_query: S) -> Result<ElementList> {
		self.select(&css_query)
			.ok_or_else(|| error!("No element found for selector: `{}`", css_query.as_ref()))
	}
}
