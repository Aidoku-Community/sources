use super::*;
use aidoku::{
	AidokuError, ContentRating, MangaStatus, MultiSelectFilter, PageContent,
	imports::html::{Document, Element, ElementList},
};
use json::home;

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
			.try_select("li.tagBtnClass > a.cate-option")?
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
		self.try_select_first("link[rel=canonical]")?
			.try_attr("abs:href")
	}

	fn title(&self, url: &str) -> Result<String> {
		self.try_select_first("div.title > h1")?
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
}

pub trait HomePage {
	fn home_layout(&self) -> Result<HomeLayout>;
}

impl HomePage for Document {
	fn home_layout(&self) -> Result<HomeLayout> {
		let json = &self
			.try_select("script")?
			.filter_map(|element| element.data())
			.find(|script| script.contains("let data = JSON.parse"))
			.ok_or_else(|| error!("No script contains `let data = JSON.parse`"))?
			.split_once(r#"JSON.parse(""#)
			.ok_or_else(|| error!(r#"String not found: `JSON.parse("`"#))?
			.1
			.split_once(r#"");"#)
			.ok_or_else(|| error!(r#"String not found: `");"#))?
			.0
			.replace(r#"\""#, r#"""#)
			.replace(r"\\", r"\")
			.replace(r"\'", "'");
		let home_layout = serde_json::from_str::<home::Root>(json)
			.map_err(AidokuError::message)?
			.into();
		Ok(home_layout)
	}
}

pub trait ChapterPage {
	fn pages(&self) -> Result<Vec<Page>>;
}

impl ChapterPage for Document {
	fn pages(&self) -> Result<Vec<Page>> {
		self.try_select("img.lazy")?
			.map(|element| {
				let url = element.try_attr("abs:data-original")?;
				let content = PageContent::Url(url, None);

				Ok(Page {
					content,
					..Default::default()
				})
			})
			.collect()
	}
}

pub trait TrySelector {
	fn try_select<T: AsRef<str>>(&self, css_query: T) -> Result<ElementList>;
	fn try_select_first<T: AsRef<str>>(&self, css_query: T) -> Result<Element>;
}

impl TrySelector for Document {
	fn try_select<T: AsRef<str>>(&self, css_query: T) -> Result<ElementList> {
		self.select(&css_query)
			.ok_or_else(|| error!("No element found for selector: `{}`", css_query.as_ref()))
	}

	fn try_select_first<T: AsRef<str>>(&self, css_query: T) -> Result<Element> {
		self.select_first(&css_query)
			.ok_or_else(|| error!("No element found for selector: `{}`", css_query.as_ref()))
	}
}

pub trait TryElement {
	fn try_attr<T: AsRef<str>>(&self, attr_name: T) -> Result<String>;
}

impl TryElement for Element {
	fn try_attr<T: AsRef<str>>(&self, attr_name: T) -> Result<String> {
		self.attr(&attr_name)
			.ok_or_else(|| error!("Attribute not found: `{}`", attr_name.as_ref()))
	}
}

#[cfg(test)]
mod test;
