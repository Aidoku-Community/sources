use super::*;
use aidoku::{
	AidokuError, SelectFilter,
	alloc::{borrow::ToOwned as _, format},
	error,
	imports::{
		html::{Document, Element, ElementList},
		js::JsContext,
	},
};
use json::MangaItem;

pub trait GenresPage {
	fn filter(&self) -> Result<SelectFilter>;
}

impl GenresPage for Document {
	fn filter(&self) -> Result<SelectFilter> {
		let (mut options, mut ids) = self
			.try_select("div#all a:not([disabled])")?
			.filter_map(|element| {
				let option = element.own_text()?.into();
				let id = element.attr("href")?.rsplit_once('=')?.1.to_owned().into();
				Some((option, id))
			})
			.collect::<(Vec<_>, Vec<_>)>();

		options.insert(0, "全部".into());
		ids.insert(0, "".into());

		Ok(SelectFilter {
			id: "題材".into(),
			title: Some("題材".into()),
			is_genre: true,
			uses_tag_style: true,
			options,
			ids: Some(ids),
			..Default::default()
		})
	}
}

pub trait FiltersPage {
	fn manga_page_result(&self) -> Result<MangaPageResult>;
}

impl FiltersPage for Document {
	fn manga_page_result(&self) -> Result<MangaPageResult> {
		let single_quoted_json = self
			.try_select_first("div.exemptComic-box")?
			.attr("list")
			.ok_or_else(|| error!("Attribute not found: `list`"))?;
		let json = JsContext::new().eval(&format!("JSON.stringify({single_quoted_json})"))?;
		let entries = serde_json::from_str::<Vec<MangaItem>>(&json)
			.map_err(AidokuError::message)?
			.into_iter()
			.map(Into::into)
			.collect();

		let has_next_page = !self
			.try_select("li.page-all-item")?
			.next_back()
			.ok_or_else(|| error!("No element found for selector: `li.page-all-item`"))?
			.has_class("active");

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
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

#[cfg(test)]
mod test;
