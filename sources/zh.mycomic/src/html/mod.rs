use crate::{BASE_URL, net::Url};
use aidoku::{
	Manga, MangaPageResult, MangaStatus, Viewer, Page, Result,
	alloc::{String, Vec, string::ToString as _, vec},
	error,
	imports::html::{Document, ElementList},
	prelude::*,
	serde::Deserialize,
};
use regex::Regex;

fn extract_chapter_number(title: &str) -> Option<f32> {
	let re =
		Regex::new(r"(?:第\s*)(\d+(?:\.\d+)?)|(\d+(?:\.\d+)?)\s*(?:话|話|章|回|卷|册|冊)").ok()?;
	if let Some(captures) = re.captures(title) {
		let num_match = captures.get(1).or_else(|| captures.get(2));
		if let Some(num_match) = num_match
			&& let Ok(num) = num_match.as_str().parse::<f32>()
		{
			return Some(num);
		}
	}
	None
}

#[derive(Deserialize)]
struct ChapterItem {
	id: u32,
	title: String,
}

type ChaptersJson = Vec<ChapterItem>;

pub trait MangaPage {
	fn update_details(&self, manga: &mut Manga) -> Result<()>;
	fn manga_page_result(&self) -> Result<MangaPageResult>;
}

impl MangaPage for Document {
	fn update_details(&self, manga: &mut Manga) -> Result<()> {
		let cdn_base = self
			.select_first("body[x-data]")
			.and_then(|body| body.attr("x-data"))
			.and_then(|data| {
				data.find("cdnUrl: '").and_then(|start| {
					data[start + 9..]
						.find("'")
						.map(|end| format!("https://{}", &data[start + 9..start + 9 + end]))
				})
			})
			.unwrap_or_else(|| "https://biccam.com".to_string());

		manga.cover = self
			.select_first("meta[name='og:image']")
			.and_then(|meta| meta.attr("content"))
			.or_else(|| {
				self.select_first("img.object-cover")
					.and_then(|img| img.attr("src"))
			})
			.map(|cover| {
				if cover.starts_with("http") {
					cover
				} else if cover.starts_with("//") {
					format!("https:{}", cover)
				} else if cover.starts_with("/") {
					format!("{}{}", cdn_base, cover)
				} else {
					cover
				}
			});

		manga.title = self
			.select_first("title")
			.and_then(|title| title.text())
			.map(|t| t.replace(" - MYCOMIC - 我的漫畫", ""))
			.unwrap_or_default();

		manga.authors = Some(vec![
			self.select_first("meta[name='author']")
				.and_then(|meta| meta.attr("content"))
				.unwrap_or_default(),
		]);

		manga.description = self
			.select_first("div[x-show='show']")
			.and_then(|div| div.text())
			.or_else(|| {
				self.select_first("meta[name='description']")
					.and_then(|meta| meta.attr("content"))
			});

		manga.status = match self
			.select_first("div[data-flux-badge]")
			.and_then(|badge| badge.text())
			.unwrap_or_default()
			.trim()
		{
			"連載中" => MangaStatus::Ongoing,
			"已完結" => MangaStatus::Completed,
			_ => MangaStatus::Unknown,
		};

		manga.url = Some(Url::manga(manga.key.clone()).to_string());

		let country = self
    		.select_first("a[href*='country']")
   			.and_then(|a| a.text());

		let tags = self
			.select("a[href*='tag']")
			.map(|elements| elements.filter_map(|a| a.text()).collect::<Vec<String>>())
			.unwrap_or_default();

		let mut all_tags = Vec::new();
		if let Some(ref c) = country {
		  	all_tags.push(c.clone());
		}
		all_tags.extend(tags);
		manga.tags = Some(all_tags);

		manga.viewer = if let Some(ref c) = country {
			if c.contains("内地") || c.contains("韩国") || c.contains("韓國") {
				Viewer::Webtoon
			} else if c.contains("日本") {
				Viewer::RightToLeft
			} else {
				Viewer::LeftToRight
			}
		} else {
			Viewer::LeftToRight
		};

		Ok(())
	}

	fn manga_page_result(&self) -> Result<MangaPageResult> {
		let mut entries: Vec<Manga> = Vec::new();

		for item in self.try_select(".group")? {
			let href = item
				.select_first("a")
				.and_then(|a| a.attr("href"))
				.unwrap_or_default();
			let id = href
				.split('/')
				.filter(|s| !s.is_empty())
				.next_back()
				.unwrap_or_default();

			let img = item.select_first("a>img");
			let cover = img
				.as_ref()
				.and_then(|img| img.attr("data-src").or_else(|| img.attr("src")))
				.unwrap_or_default();
			let title = img
				.as_ref()
				.and_then(|img| img.attr("alt"))
				.unwrap_or_default();

			if !id.is_empty() && !title.is_empty() {
				entries.push(Manga {
					key: id.to_string(),
					cover: (!cover.is_empty()).then_some(cover),
					title,
					..Default::default()
				});
			}
		}

		let has_next_page = self.select_first("a[rel='next']").is_some();

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
		let mut chapters: Vec<aidoku::Chapter> = Vec::new();

		for element in self.try_select("div[x-data*='chapters']")? {
			let scanlator = element
				.select_first("div[data-flux-subheading] div")
				.and_then(|div| div.text())
				.unwrap_or_default()
				.trim()
				.to_string();

			let data = element.attr("x-data").unwrap_or_default();
			let text = if let Some(start) = data.find("chapters:") {
				if let Some(end) = data[start..].find("],") {
					data[start + 9..start + end + 1].trim().to_string()
				} else {
					continue;
				}
			} else {
				continue;
			};

			// Parse JSON array of chapter objects
			let chapters_data: ChaptersJson = serde_json::from_str(&text).unwrap_or_default();
			let len = chapters_data.len();

			for (index, chapter_item) in chapters_data.into_iter().enumerate() {
				let chapter_id = chapter_item.id.to_string();
				let title = chapter_item.title;

				let chapter_num = (len - index) as f32;
				let chapter_or_volume = extract_chapter_number(&title).unwrap_or(chapter_num);
				let (ch, vo) = if scanlator == "单行本" || scanlator == "單行本" {
					(-1.0, chapter_or_volume)
				} else {
					(chapter_or_volume, -1.0)
				};

				let chapter_url = format!("{}/chapters/{}", BASE_URL, chapter_id);
				chapters.push(aidoku::Chapter {
					key: chapter_id,
					title: Some(title),
					volume_number: (vo >= 0.0).then_some(vo),
					chapter_number: (ch >= 0.0).then_some(ch),
					url: Some(chapter_url),
					scanlators: (!scanlator.is_empty()).then(|| vec![scanlator.clone()]),
					..Default::default()
				});
			}
		}

		Ok(chapters)
	}
}

pub trait PageList {
	fn pages(&self) -> Result<Vec<Page>>;
}

impl PageList for Document {
	fn pages(&self) -> Result<Vec<Page>> {
		let mut pages: Vec<Page> = Vec::new();
		for item in self.try_select("img.page")? {
			if let Some(url) = item.attr("data-src").or_else(|| item.attr("src"))
				&& !url.is_empty()
			{
				pages.push(Page {
					content: aidoku::PageContent::Url(url, None),
					..Default::default()
				});
			}
		}
		Ok(pages)
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
