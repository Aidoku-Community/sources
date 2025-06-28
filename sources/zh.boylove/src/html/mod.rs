use super::*;
use aidoku::{AidokuError, ContentRating, MangaStatus, MultiSelectFilter, imports::html::Document};
use json::chapter_list;

pub trait FiltersPage {
	fn tags_filter(&self) -> Result<Filter>;
}

impl FiltersPage for Document {
	fn tags_filter(&self) -> Result<Filter> {
		let id = "標籤".into();

		let title = "標籤".into();

		let is_genre = true;

		let uses_tag_style = true;

		let options = self
			.select("li.tagBtnClass > a.cate-option")
			.ok_or_else(|| {
				error!("No element found for selector: `li.tagBtnClass > a.cate-option`")
			})?
			.filter_map(|element| {
				element
					.attr("data-value")
					.filter(|data_value| !matches!(data_value.as_str(), "0" | "待分類" | "待分类"))
					.map(Into::into)
			})
			.collect();

		let filter = MultiSelectFilter {
			id,
			title: Some(title),
			is_genre,
			uses_tag_style,
			options,
			..Default::default()
		}
		.into();
		Ok(filter)
	}
}

pub trait MangaPage {
	fn manga_details(&self) -> Result<Manga>;
	fn url(&self) -> Result<String>;
	fn title(&self, url: &str) -> Result<String>;
	fn cover(&self) -> Option<String>;
	fn authors(&self) -> Option<Vec<String>>;
	fn description(&self) -> Option<String>;
	fn tags(&self) -> Option<Vec<String>>;
	fn status(&self) -> MangaStatus;
	fn chapters(&self) -> Result<Option<Vec<Chapter>>>;
}

impl MangaPage for Document {
	fn manga_details(&self) -> Result<Manga> {
		let url = self.url()?;

		let key = url
			.rsplit_once('/')
			.ok_or_else(|| error!("No character `/` found in URL: `{url}`"))?
			.1
			.into();

		let title = self.title(&url)?;

		let cover = self.cover();

		let authors = self.authors();

		let description = self.description();

		let tags = self.tags();

		let status = self.status();

		let content_rating = tags
			.as_deref()
			.and_then(|tags_slice| {
				tags_slice
					.iter()
					.any(|tag| tag == "清水")
					.then_some(ContentRating::Safe)
			})
			.unwrap_or(ContentRating::NSFW);

		Ok(Manga {
			key,
			title,
			cover,
			authors,
			description,
			url: Some(url),
			tags,
			status,
			content_rating,
			..Default::default()
		})
	}

	fn url(&self) -> Result<String> {
		self.select_first("link[rel=canonical]")
			.ok_or_else(|| error!("No element found for selector: `link[rel=canonical]`"))?
			.attr("abs:href")
			.ok_or_else(|| error!("Attribute not found: `href`"))
	}

	fn title(&self, url: &str) -> Result<String> {
		self.select_first("div.title > h1")
			.ok_or_else(|| error!("No element found for selector: `div.title > h1`"))?
			.text()
			.ok_or_else(|| error!("No title found for URL: {url}"))
	}

	fn cover(&self) -> Option<String> {
		self.select_first("a.play")?.attr("abs:data-original")
	}

	fn authors(&self) -> Option<Vec<String>> {
		let authors = self
			.select("p.data:contains(作者) > a")?
			.filter_map(|element| element.text())
			.collect();
		Some(authors)
	}

	fn description(&self) -> Option<String> {
		let html = self.select_first("span.detail-text")?.html()?;
		let description = html
			.split_once("</")
			.map(|(description, _)| description.into())
			.unwrap_or(html)
			.split("<br />")
			.map(str::trim)
			.collect::<Vec<_>>()
			.join("  \n")
			.trim()
			.into();
		Some(description)
	}

	fn tags(&self) -> Option<Vec<String>> {
		let tags = self
			.select("p.data:contains(标签) > a.tag span")?
			.filter_map(|element| element.text())
			.filter(|tag| !tag.is_empty())
			.collect();
		Some(tags)
	}

	fn status(&self) -> MangaStatus {
		match self
			.select_first("p.data:not(:has(*))")
			.and_then(|element| element.text())
			.as_deref()
		{
			Some("连载中" | "連載中") => MangaStatus::Ongoing,
			Some("完结" | "完結") => MangaStatus::Completed,
			_ => MangaStatus::Unknown,
		}
	}

	fn chapters(&self) -> Result<Option<Vec<Chapter>>> {
		let Some(json) = self.select("script").and_then(|elements| {
			let json = elements
				.filter_map(|element| element.data())
				.find(|script| script.contains("function getChapterList"))?
				.split_once("function getChapterList")?
				.1
				.split_once('"')?
				.1
				.split_once(r#"");"#)?
				.0
				.replace(r#"\""#, r#"""#)
				.replace(r"\\", r"\");
			Some(json)
		}) else {
			return Ok(None);
		};
		let chapters = serde_json::from_str::<chapter_list::Root>(&json)
			.map_err(AidokuError::message)?
			.try_into()?;
		Ok(Some(chapters))
	}
}

#[cfg(test)]
mod test;
