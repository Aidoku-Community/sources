use crate::{BASE_URL, net::Url};
use aidoku::{
	Manga, MangaPageResult, MangaStatus, Viewer, Page, Result,
	alloc::{String, Vec, string::ToString as _},
	error,
	imports::{
		html::{Document, Element, ElementList},
		net::Request,
	},
	prelude::*,
};
use regex::Regex;

fn extract_chapter_number(title: &str) -> Option<f32> {
	let re = Regex::new(r"(?:第\s*)([\d０-９]+(?:\.[\d０-９]+)?)|([\d０-９]+(?:\.[\d０-９]+)?)\s*(?:话|話|章|回|卷|册|冊)").ok()?;
	if let Some(captures) = re.captures(title) {
		let num_match = captures.get(1).or_else(|| captures.get(2));
		if let Some(num_match) = num_match {
			let num_str = num_match
				.as_str()
				.chars()
				.map(|c| match c {
					'０' => '0',
					'１' => '1',
					'２' => '2',
					'３' => '3',
					'４' => '4',
					'５' => '5',
					'６' => '6',
					'７' => '7',
					'８' => '8',
					'９' => '9',
					'．' => '.',
					other => other,
				})
				.collect::<String>();
			if let Ok(num) = num_str.parse::<f32>() {
				return Some(num);
			}
		}
	}
	None
}

fn extract_chapter_key(href: &str) -> String {
	href.split("/")
		.filter(|s| !s.is_empty())
		.last()
		.unwrap_or("")
		.replace(".html", "")
}

fn create_chapter(
	chapter_href: String,
	title: String,
	volume_num: f32,
	chapters_len: usize,
	volume_thumbnail: Option<String>,
) -> aidoku::Chapter {
	let chapter_key = extract_chapter_key(&chapter_href);
	let chapter_num = extract_chapter_number(&title).unwrap_or(chapters_len as f32 + 1.0);
	let url = format!("{}{}", BASE_URL, chapter_href);

	aidoku::Chapter {
		key: chapter_key,
		title: Some(title),
		volume_number: Some(volume_num),
		chapter_number: Some(chapter_num),
		url: Some(url),
		thumbnail: volume_thumbnail,
		..Default::default()
	}
}

pub trait MangaPage {
	fn update_details(&self, manga: &mut Manga) -> Result<()>;
	fn manga_page_result(&self) -> Result<MangaPageResult>;
}

impl MangaPage for Document {
	fn update_details(&self, manga: &mut Manga) -> Result<()> {
		manga.cover = self.try_select_first(".book-cover")?.attr("src");
		manga.title = self
			.try_select_first("h1.book-title")?
			.text()
			.unwrap_or_default();
		let authors = self
			.try_select(".authorname,.illname")?
			.filter_map(|a| a.text())
			.collect::<Vec<String>>();
		manga.authors = Some(authors);
		manga.description = Some(
			self.try_select_first(".book-summary>content")?
				.text()
				.unwrap_or_default(),
		);
		let tags = self
			.try_select(".tag-small-group>.tag-small>a")?
			.filter_map(|a| a.text())
			.collect::<Vec<String>>();
		manga.tags = Some(tags);
		manga.status = match self
			.try_select_first(".book-layout-inline")?
			.text()
			.unwrap_or_default()
			.trim()
			.split("|")
			.map(|a| a.trim().to_string())
			.collect::<Vec<String>>()
			.first()
			.map(|s| s.as_str())
			.unwrap_or("")
		{
			"連載" => MangaStatus::Ongoing,
			"完結" => MangaStatus::Completed,
			_ => MangaStatus::Unknown,
		};
		let tags = manga.tags.as_deref().unwrap_or(&[]);
		manga.viewer = if tags
			.iter()
			.any(|tag| tag.contains("大陸") || tag.contains("韓國"))
		{
			Viewer::Webtoon
		} else if tags
			.iter()
			.any(|tag| tag.contains("日本"))
		{
			Viewer::RightToLeft
		} else {
			Viewer::LeftToRight
		};
		manga.url = Some(Url::manga(manga.key.clone()).to_string());
		Ok(())
	}
	fn manga_page_result(&self) -> Result<MangaPageResult> {
		let mut entries: Vec<Manga> = Vec::new();

		let alternate_url = self
			.select_first("link[rel='alternate']")
			.and_then(|link| link.attr("href"))
			.unwrap_or_default();

		if alternate_url.contains("detail") {
			let key = alternate_url
				.split("/")
				.filter(|s| !s.is_empty())
				.last()
				.unwrap_or("")
				.replace(".html", "");

			let cover = self.try_select_first(".book-cover")?.attr("src");
			let title = self
				.try_select_first("h1.book-title")?
				.text()
				.unwrap_or_default();

			entries.push(Manga {
				key,
				cover,
				title,
				..Default::default()
			});
		} else {
			let items = self.try_select(".book-li>a")?;
			for item in items {
				let href = item.attr("href").unwrap_or_default();
				let key = href
					.split("/")
					.filter(|s| !s.is_empty())
					.last()
					.unwrap_or("")
					.replace(".html", "");

				let cover = item
					.select_first(".book-cover>img")
					.and_then(|img| img.attr("data-src"));
				let title = item
					.select_first(".book-title")
					.and_then(|title| title.text())
					.unwrap_or_default();

				if !key.is_empty() && !title.is_empty() {
					entries.push(Manga {
						key,
						cover,
						title,
						..Default::default()
					});
				}
			}
		}

		let has_next_page = self
			.select_first("#pagelink")
			.and_then(|pagelink| {
				let strong_text = pagelink
					.select_first("strong")
					.and_then(|s| s.text());
				let last_text = pagelink
					.select_first(".last")
					.and_then(|l| l.text());
				if let (Some(current), Some(last)) = (strong_text, last_text) {
					Some(current != last)
				} else {
					pagelink
						.select_first(".next")
						.and_then(|n| n.attr("href"))
						.map(|href| href != "#")
				}
			})
			.unwrap_or(false);

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}
}

pub trait ChapterPage {
	fn chapters(&self, manga_id: &str) -> Result<Vec<aidoku::Chapter>>;
}

impl ChapterPage for Document {
	fn chapters(&self, _manga_id: &str) -> Result<Vec<aidoku::Chapter>> {
		let volumes = self.try_select(".catalog-volume")?;
		let mut chapters: Vec<aidoku::Chapter> = Vec::new();

		for volume in volumes {
			let volume_title = volume
				.select("h3")
				.and_then(|h3| h3.text())
				.unwrap_or_default();
			let volume_num = extract_chapter_number(&volume_title).unwrap_or(-1.0);
			let chapter_links = volume.select(".chapter-li-a");

			if let Some(chapter_links) = chapter_links
				&& !chapter_links.is_empty()
			{
				let mut has_javascript_link = false;
				let mut i = 0;
				while let Some(link) = chapter_links.get(i) {
					if link
						.attr("href")
						.is_some_and(|href| href.starts_with("javascript:"))
					{
						has_javascript_link = true;
						break;
					}
					i += 1;
				}
				let volume_thumbnail = volume
					.select_first(".volume-cover-img img")
					.and_then(|img| img.attr("data-src"));

				let chapter_items = if has_javascript_link {
					let vol_href = volume
						.select_first(".volume-cover-img")
						.and_then(|v| v.attr("href"))
						.unwrap_or_default();
					let vol_url = format!("{}{}", BASE_URL, vol_href);
					let vol_html = Request::get(vol_url)?
						.header("Origin", BASE_URL)
						.html()?;
					vol_html.try_select(".catalog-volume .chapter-li-a")?
				} else {
					chapter_links
				};

				for item in chapter_items {
					let chapter_href = item.attr("href").unwrap_or_default();
					let title = item
						.select_first("span")
						.and_then(|span| span.text())
						.unwrap_or_default();
					let chapter = create_chapter(
						chapter_href,
						title,
						volume_num,
						chapters.len(),
						volume_thumbnail.clone(),
					);
					chapters.push(chapter);
				}
			}
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
		for item in self.try_select("#acontentz>img")? {
			let url = item
				.attr("data-src")
				.unwrap_or_default()
				.trim()
				.to_string();
			pages.push(Page {
				content: aidoku::PageContent::Url(url, None),
				..Default::default()
			});
		}
		Ok(pages)
	}
}

trait TrySelect {
	fn try_select<S: AsRef<str>>(&self, css_query: S) -> Result<ElementList>;
	fn try_select_first<S: AsRef<str>>(&self, css_query: S) -> Result<Element>;
}

impl TrySelect for Document {
	fn try_select<S: AsRef<str>>(&self, css_query: S) -> Result<ElementList> {
		self.select(&css_query)
			.ok_or_else(|| error!("No element found for selector: `{}`", css_query.as_ref()))
	}

	fn try_select_first<S: AsRef<str>>(&self, css_query: S) -> Result<Element> {
		self.select_first(&css_query)
			.ok_or_else(|| error!("No element found for selector: `{}`", css_query.as_ref()))
	}
}
