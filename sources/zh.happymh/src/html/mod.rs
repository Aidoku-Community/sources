use crate::BASE_URL;
use aidoku::{
	alloc::{string::ToString as _, vec, String, Vec},
	error,
	imports::{
		html::{Document, ElementList},
		net::Request,
	},
	prelude::*,
	Manga, MangaPageResult, Result, Viewer,
};

pub trait MangaPage {
	fn update_details(&self, manga: &mut Manga) -> Result<()>;
	fn manga_page_result(&self) -> Result<MangaPageResult>;
}

impl MangaPage for Document {
	fn update_details(&self, manga: &mut Manga) -> Result<()> {
		let url = format!("{}/manga/{}", BASE_URL, manga.key);
		let html = Request::get(url.clone())?
			.header("Origin", BASE_URL)
			.html()?;

		manga.cover = html
			.select_first(".mg-cover>mip-img")
			.and_then(|e| e.attr("src"));
		manga.title = html
			.select_first("h2.mg-title")
			.and_then(|e| e.text())
			.unwrap_or_default();
		let author = html
			.select(".mg-sub-title>a")
			.map(|elements| {
				elements
					.filter_map(|a| a.text())
					.collect::<Vec<String>>()
					.join(", ")
			})
			.unwrap_or_default();
		let description = html
			.select_first("#showmore")
			.and_then(|e| e.text())
			.map(|t| t.trim().to_string())
			.unwrap_or_default();
		let categories = html
			.select(".mg-cate>a")
			.map(|elements| elements.filter_map(|a| a.text()).collect::<Vec<String>>())
			.unwrap_or_default();

		manga.authors = Some(vec![author]);
		manga.description = Some(description);
		manga.tags = Some(categories);
		manga.viewer = Viewer::Webtoon;
		manga.url = Some(url);

		Ok(())
	}

	fn manga_page_result(&self) -> Result<MangaPageResult> {
		let entries: Vec<Manga> = self
			.try_select(".manga-rank")?
			.filter_map(|item| {
				let id = item
					.select_first(".manga-rank-cover>a")
					.and_then(|e| e.attr("href"))
					.and_then(|href| {
						href.split("/")
							.filter(|a| !a.is_empty())
							.last()
							.map(|s| s.to_string())
					})?;
				let cover = item
					.select_first(".manga-rank-cover>a>mip-img")
					.and_then(|e| e.attr("src"))?;
				let title = item
					.select_first(".manga-title")
					.and_then(|e| e.text())
					.map(|t| t.trim().to_string())?;

				Some(Manga {
					key: id,
					cover: Some(cover),
					title,
					..Default::default()
				})
			})
			.collect();

		Ok(MangaPageResult {
			entries,
			has_next_page: false,
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
