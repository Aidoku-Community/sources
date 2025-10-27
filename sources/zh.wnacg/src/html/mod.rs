use crate::net::Url;
use aidoku::{
	Manga, MangaPageResult, MangaStatus, Result, Viewer,
	alloc::{String, Vec, string::ToString as _},
	error,
	imports::{
		html::{Document, Element, ElementList},
		std::parse_date,
	},
	prelude::*,
};

pub trait MangaPage {
	fn manga_page_result(&self) -> Result<MangaPageResult>;
	fn manga_details(&self, manga: &mut Manga) -> Result<()>;
}

impl MangaPage for Document {
	fn manga_page_result(&self) -> Result<MangaPageResult> {
		let entries: Vec<Manga> = self
			.try_select(".gallary_item")?
			.filter_map(|item| {
				let href = item.select_first(".pic_box>a")?.attr("href")?;
				let id = href
					.split('-')
					.filter(|s| !s.is_empty())
					.next_back()?
					.replace(".html", "");

				let cover = item
					.select_first(".pic_box>a>img")?
					.attr("src")
					.map(|src| format!("https:{}", src))?;

				let title = item
					.select_first(".info>.title>a")?
					.text()?
					.trim()
					.to_string();

				let description = item
					.select_first(".info>.info_col")?
					.text()?
					.replace("創建於", "")
					.replace("張照片", "P")
					.replace(", ", "  \n")
					.replace("， ", "  \n")
					.trim()
					.to_string();

				Some(Manga {
					key: id,
					cover: Some(cover),
					title,
					description: Some(description),
					..Default::default()
				})
			})
			.collect();

		let has_next_page = self
			.select_first(".bot_toolbar .paginator .next")
			.is_some();

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn manga_details(&self, manga: &mut Manga) -> Result<()> {
		let cover = self
			.try_select_first("#bodywrap>div>.uwthumb>img")?
			.attr("src")
			.ok_or_else(|| error!("No src attribute found"))?
			.replace("//", "");
		manga.cover = Some(format!("https://{}", cover));

		let title = self
			.try_select_first("#bodywrap>h2")?
			.text()
			.unwrap_or_default();

		let categories = self
			.try_select_first("#bodywrap>div>.uwconn>label:nth-child(1)")?
			.text()
			.unwrap_or_default()
			.replace("分類：", "")
			.split("／")
			.flat_map(|a| a.split("&"))
			.map(|a| a.trim().to_string())
			.collect::<Vec<String>>();

		let tags = self
			.try_select("#bodywrap>div>.uwconn>.addtags>.tagshow")?
			.filter_map(|tag| tag.text())
			.collect::<Vec<String>>();

		manga.viewer = if categories.iter().any(|tag| tag.contains("韓漫")) {
			Viewer::Webtoon
		} else {
			Viewer::RightToLeft
		};

		let mut all_tags = categories;
		all_tags.extend(tags);
		manga.tags = Some(all_tags);

		let description = self
			.try_select_first("#bodywrap>div>.uwconn>p")?
			.html()
			.map(|html| {
				// Remove the "簡介：" prefix if it exists
				let cleaned = html.replace("簡介：", "");
				// Remove the first * if it exists at the beginning
				let cleaned = if let Some(stripped) = cleaned.strip_prefix('*') {
					stripped.to_string()
				} else {
					cleaned
				};
				// Normalize all variations of <br> tags to a single marker
				let cleaned = cleaned.replace("<br/>", "<br>").replace("<br />", "<br>");

				// Replace sequences of <br> tags (with optional whitespace) with a single newline
				let mut result = String::new();
				let parts: Vec<&str> = cleaned.split("<br>").collect();

				for part in parts.iter() {
					let trimmed = part.trim();
					if !trimmed.is_empty() {
						if !result.is_empty() {
							result.push_str("  \n");
						}
						result.push_str(trimmed);
					}
				}
				result
			})
			.unwrap_or_default();

		// Get page count information
		let page_info = self
			.try_select_first("#bodywrap>div>.uwconn>label:nth-child(2)")?
			.text()
			.unwrap_or_default();

		let full_description = if !description.is_empty() {
			format!("{}  \n簡介：{}", page_info, description)
		} else {
			page_info
		};

		manga.status = if title.contains("[完結]") {
			MangaStatus::Completed
		} else {
			MangaStatus::Unknown
		};
		manga.title = title;
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
		let date_uploaded = self.try_select_first(".info_col")?.text().and_then(|text| {
			// Parse "上傳於2025-10-15" format
			if text.contains("上傳於") {
				let date_str = text.replace("上傳於", "").trim().to_string();
				parse_date(date_str, "yyyy-MM-dd")
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
