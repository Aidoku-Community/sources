use super::*;
use aidoku::{
	SelectFilter,
	alloc::borrow::ToOwned as _,
	error,
	imports::html::{Document, ElementList},
};

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

trait TrySelect {
	fn try_select<S: AsRef<str>>(&self, css_query: S) -> Result<ElementList>;
}

impl TrySelect for Document {
	fn try_select<S: AsRef<str>>(&self, css_query: S) -> Result<ElementList> {
		self.select(&css_query)
			.ok_or_else(|| error!("No element found for selector: `{}`", css_query.as_ref()))
	}
}

#[cfg(test)]
mod test;
