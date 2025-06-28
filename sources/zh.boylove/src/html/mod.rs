use super::*;
use aidoku::{MultiSelectFilter, error, imports::html::Document};

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

#[cfg(test)]
mod test;
