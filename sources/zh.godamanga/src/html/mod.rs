use crate::{handle_cover_url, BASE_URL};
use aidoku::{
	alloc::{string::ToString as _, vec, String, Vec},
	error,
	imports::{
		html::{Document, ElementList},
		net::Request,
	},
	prelude::*,
	Manga, MangaPageResult, MangaStatus, Result, Viewer,
};

pub trait MangaPage {
	fn update_details(&self, manga: &mut Manga) -> Result<()>;
	fn manga_page_result(&self) -> Result<MangaPageResult>;
}

impl MangaPage for Document {
	fn update_details(&self, manga: &mut Manga) -> Result<()> {
		let ids = manga.key.split("/").collect::<Vec<&str>>();
		let url = format!("{}/manga/{}", BASE_URL, ids[0]);
		let html = Request::get(&url)?.header("Origin", BASE_URL).html()?;

		let mid = html
			.select_first("#mangachapters")
			.and_then(|e| e.attr("data-mid"))
			.unwrap_or_default();
		manga.cover = html
			.select_first("meta[property='og:image']")
			.and_then(|e| e.attr("content"))
			.map(handle_cover_url);
		manga.title = html
			.select_first("title")
			.and_then(|e| e.text())
			.map(|t| t.replace("-G站漫畫", ""))
			.unwrap_or_default();
		let author = html
			.select("a[href*=author]>span")
			.map(|elements| {
				elements
					.filter_map(|a| a.text().map(|t| t.replace(",", "")))
					.filter(|a| !a.is_empty())
					.collect::<Vec<String>>()
					.join(", ")
			})
			.unwrap_or_default();
		let description = html
			.select_first(".text-medium.my-unit-md")
			.and_then(|e| e.text());
		let categories = html
			.select(".py-1>a:not([href*=author])>span")
			.map(|elements| {
				elements
					.filter_map(|a| {
						a.text().map(|t| {
							t.replace(",", "")
								.replace("热门漫画", "")
								.replace("#", "")
								.replace("热门推荐", "")
								.trim()
								.to_string()
						})
					})
					.filter(|a| !a.is_empty())
					.collect::<Vec<String>>()
			});
		let status = html
			.select_first("h1 span")
			.and_then(|span| span.text())
			.map(|text| match text.trim() {
				"完結" | "已完結" => MangaStatus::Completed,
				"連載中" => MangaStatus::Ongoing,
				"停止更新" => MangaStatus::Cancelled,
				"休刊" => MangaStatus::Hiatus,
				_ => MangaStatus::Ongoing,
			})
			.unwrap_or(MangaStatus::Ongoing);

		manga.key = format!("{}/{}", ids[0], mid);
		manga.authors = Some(vec![author]);
		manga.description = description;
		manga.tags = categories;
		manga.status = status;
		manga.url = Some(url);

		// Set viewer based on categories
		let categories = manga.tags.as_deref().unwrap_or(&[]);
		manga.viewer = if categories
			.iter()
			.any(|tag| tag.contains("国漫") || tag.contains("韩漫"))
		{
			Viewer::Webtoon
		} else if categories.iter().any(|tag| tag.contains("日漫")) {
			Viewer::RightToLeft
		} else {
			Viewer::LeftToRight
		};

		Ok(())
	}

	fn manga_page_result(&self) -> Result<MangaPageResult> {
		let mut entries: Vec<Manga> = Vec::new();

		for item in self.try_select(".pb-2>a")? {
			let href = item.attr("href").unwrap_or_default();
			let id = href
				.split("/")
				.filter(|s| !s.is_empty())
				.last()
				.unwrap_or_default()
				.to_string();
			let cover = handle_cover_url(
				item.select_first("div>img")
					.and_then(|img| img.attr("src"))
					.unwrap_or_default(),
			);
			let title = item
				.select_first("div>h3")
				.and_then(|h3| h3.text())
				.unwrap_or_default();

			if !id.is_empty() && !title.is_empty() {
				entries.push(Manga {
					key: id,
					cover: Some(cover),
					title,
					..Default::default()
				});
			}
		}

		let has_next_page = self
			.select_first("a[href*='/page/'][aria-label='下一頁']")
			.is_some();

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
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

