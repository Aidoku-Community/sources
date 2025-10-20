use crate::net::Url;
use aidoku::{
	Manga, MangaPageResult, MangaStatus, Viewer, Result,
	alloc::{String, Vec, string::ToString as _},
	error,
	imports::html::{Document, Element, ElementList},
	prelude::*,
};
use chrono::DateTime;

pub trait MangaPage {
	fn manga_page_result(&self) -> Result<MangaPageResult>;
	fn manga_details(&self, manga: &mut Manga) -> Result<()>;
}

impl MangaPage for Document {
	fn manga_page_result(&self) -> Result<MangaPageResult> {
		let mut entries: Vec<Manga> = Vec::new();

		for item in self.try_select(".gallary_item")? {
			let href = item
				.select_first(".pic_box>a")
				.and_then(|a| a.attr("href"))
				.unwrap_or_else(String::new);
			let id = href
				.split('-')
				.filter(|s| !s.is_empty())
				.next_back()
				.unwrap_or("")
				.replace(".html", "");

			let cover = item
				.select_first(".pic_box>a>img")
				.and_then(|img| img.attr("src"))
				.map(|src| format!("https:{}", src))
				.unwrap_or_else(String::new);

			let title = item
				.select_first(".info>.title>a")
				.and_then(|a| a.text())
				.unwrap_or_else(String::new)
				.trim()
				.to_string();

			let description = item
				.select_first(".info>.info_col")
				.and_then(|div| div.text())
				.unwrap_or_else(String::new)
				.replace("創建於", "")
				.replace("張照片", "P")
				.replace(", ", "  \n")
				.replace("， ", "  \n")
				.trim()
				.to_string();

			if !id.is_empty() && !title.is_empty() {
				entries.push(Manga {
					key: id,
					cover: Some(cover),
					title,
					description: Some(description),
					..Default::default()
				});
			}
		}

		let has_next_page = true; // wnacg seems to always have more pages

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn manga_details(&self, manga: &mut Manga) -> Result<()> {
		let cover = self
			.try_select_first("#bodywrap>div>.uwthumb>img")?
			.attr("src")
			.unwrap_or_else(String::new)
			.replace("//", "");
		manga.cover = Some(format!("https://{}", cover));

		let title = self
			.try_select_first("#bodywrap>h2")?
			.text()
			.unwrap_or_else(String::new);

		let categories = self
			.try_select_first("#bodywrap>div>.uwconn>label:nth-child(1)")?
			.text()
			.unwrap_or_else(String::new)
			.replace("分類：", "")
			.split("／")
			.flat_map(|a| a.split("&"))
			.map(|a| a.trim().to_string())
			.collect::<Vec<String>>();

		let tags = self
			.try_select("#bodywrap>div>.uwconn>.addtags>.tagshow")?
			.filter_map(|tag| tag.text())
			.collect::<Vec<String>>();

		let mut all_tags = categories.clone();
		all_tags.extend(tags);
		manga.tags = Some(all_tags);

		manga.viewer = if categories.iter().any(|tag| tag.contains("韓漫")) {
			Viewer::Webtoon
		} else {
			Viewer::RightToLeft
		};

		let description = self
			.try_select_first("#bodywrap>div>.uwconn>p")?
			.text()
			.map(|text| {
				let cleaned = text.replace("簡介：", "");
				// Remove the first * if it exists at the beginning
				let cleaned = if let Some(stripped) = cleaned.strip_prefix('*') {
					stripped.to_string()
				} else {
					cleaned
				};
				cleaned
					.replace("<br />", "\n")
					.replace("<br>", "\n")
					.trim()
					.to_string()
			})
			.unwrap_or_else(String::new);

		// Get page count information
		let page_info = self
			.try_select_first("#bodywrap>div>.uwconn>label:nth-child(2)")?
			.text()
			.unwrap_or_else(String::new);

		let full_description = if !description.is_empty() {
			format!("{}\n\n{}", description, page_info)
		} else {
			page_info
		};

		manga.title = title.clone();
		manga.status = if title.contains("[完結]") {
			MangaStatus::Completed
		} else {
			MangaStatus::Unknown
		};
		manga.description = Some(full_description);
		// Status is already set above based on title
		manga.update_strategy = aidoku::UpdateStrategy::Never;
		manga.url = Some(Url::manga(manga.key.clone()).to_string());

		Ok(())
	}
}

pub trait ChapterPage {
	fn chapters(&self, manga_id: &str) -> Result<Vec<aidoku::Chapter>>;
}

impl ChapterPage for Document {
	fn chapters(&self, manga_id: &str) -> Result<Vec<aidoku::Chapter>> {
		let mut chapters: Vec<aidoku::Chapter> = Vec::new();

		let url = Url::manga(manga_id.to_string()).to_string();

		// Try to get upload date from the page
		let date_uploaded = self
			.try_select_first(".info_col")?
			.text()
			.and_then(|text| {
				// Parse "上傳於2025-10-15" format
				if text.contains("上傳於") {
					let date_str = text.replace("上傳於", "").trim().to_string();
					// Convert to RFC3339 format for parsing
					let rfc3339 = format!("{}T00:00:00Z", date_str);
					DateTime::parse_from_rfc3339(&rfc3339)
						.ok()
						.map(|dt| dt.timestamp())
				} else {
					None
				}
			});

		chapters.push(aidoku::Chapter {
			key: manga_id.to_string(),
			chapter_number: Some(1.0),
			url: Some(url),
			date_uploaded,
			..Default::default()
		});

		Ok(chapters)
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
