use crate::{handle_cover_url, BASE_URL};
use aidoku::{
	alloc::{string::ToString as _, vec, String, Vec},
	error,
	imports::{
		html::{Document, ElementList},
		net::{HttpMethod, Request},
	},
	prelude::*,
	Manga, MangaPageResult, MangaStatus, Result,
};

pub trait MangaPage {
	fn update_details(&self, manga: &mut Manga) -> Result<()>;
	fn manga_page_result(&self) -> Result<MangaPageResult>;
}

impl MangaPage for Document {
	fn update_details(&self, manga: &mut Manga) -> Result<()> {
		let url = format!("{}/manga/{}", BASE_URL, manga.key);
		let html = Request::new(url.clone(), HttpMethod::Get)?
			.header("Origin", BASE_URL)
			.html()?;

		manga.cover = html
			.select_first(".mg-cover>mip-img")
			.and_then(|e| e.attr("src"))
			.map(handle_cover_url);
		manga.title = html
			.select_first("h2.mg-title")
			.and_then(|e| e.text())
			.unwrap_or_else(String::new);
		let author = html
			.select(".mg-sub-title>a")
			.map(|elements| {
				elements
					.filter_map(|a| a.text())
					.collect::<Vec<String>>()
					.join(", ")
			})
			.unwrap_or_else(String::new);
		let description = html
			.select_first("#showmore")
			.and_then(|e| e.text())
			.map(|t| t.trim().to_string())
			.unwrap_or_else(String::new);
		let categories = html
			.select(".mg-cate>a")
			.map(|elements| elements.filter_map(|a| a.text()).collect::<Vec<String>>())
			.unwrap_or_else(Vec::new);
		let status = MangaStatus::Unknown;

		manga.authors = Some(vec![author]);
		manga.description = Some(description);
		manga.tags = Some(categories);
		manga.status = status;
		manga.url = Some(url);

		Ok(())
	}

	fn manga_page_result(&self) -> Result<MangaPageResult> {
		let mut entries: Vec<Manga> = Vec::new();

		for item in self.try_select(".manga-rank")? {
			let id = item
				.select_first(".manga-rank-cover>a")
				.and_then(|e| e.attr("href"))
				.and_then(|href| {
					href.split("/")
						.filter(|a| !a.is_empty())
						.last()
						.map(|s| s.to_string())
				})
				.unwrap_or_else(String::new);
			let cover = item
				.select_first(".manga-rank-cover>a>mip-img")
				.and_then(|e| e.attr("src"))
				.map(handle_cover_url)
				.unwrap_or_else(String::new);
			let title = item
				.select_first(".manga-title")
				.and_then(|e| e.text())
				.map(|t| t.trim().to_string())
				.unwrap_or_else(String::new);

			if !id.is_empty() && !title.is_empty() {
				entries.push(Manga {
					key: id,
					cover: Some(cover),
					title,
					..Default::default()
				});
			}
		}

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
