use crate::{
	OniSaga,
	api::{SearchParams, parse, rpc},
	helpers::{DEMOGRAPHICS, form_slug, origin_slug, slugify},
	models::TermOption,
};
use aidoku::{
	DynamicFilters, DynamicSettings, Filter, GroupSetting, MultiSelectFilter, MultiSelectSetting,
	Result, Setting,
	alloc::{String, Vec, vec},
};
use serde_json::json;

impl OniSaga {
	fn taxonomy(&self) -> Result<Vec<TermOption>> {
		if let Some(cached) = self.taxonomy.borrow().as_ref() {
			return Ok(cached.clone());
		}
		let terms: Vec<TermOption> = parse(rpc("catalog/taxonomy", json!({}))?.send()?)?;
		*self.taxonomy.borrow_mut() = Some(terms.clone());
		Ok(terms)
	}

	fn genres(&self) -> Result<(Vec<String>, Vec<String>)> {
		Ok(self
			.taxonomy()?
			.into_iter()
			.filter(|t| t.group == "genre" && !t.slug.is_empty() && !t.name.is_empty())
			.map(|t| (t.slug, t.name))
			.unzip())
	}

	pub(crate) fn resolve_tapped_tags(&self, tapped: Vec<String>, search: &mut SearchParams) {
		let mut origins = Vec::new();
		let mut unknown = Vec::new();
		for (name, slug) in tapped.into_iter().filter_map(|name| {
			let slug = slugify(&name);
			(!slug.is_empty()).then_some((name, slug))
		}) {
			if let Some(origin) = origin_slug(&slug) {
				origins.push(origin.into());
			} else if DEMOGRAPHICS.contains(&slug.as_str()) {
				search.demographic.push(slug);
			} else if let Some(form) = form_slug(&slug) {
				search.form.push(form.into());
			} else {
				unknown.push((name, slug));
			}
		}
		if !origins.is_empty() {
			search.origin = Some(origins);
		}
		if unknown.is_empty() {
			return;
		}
		let taxonomy = self.taxonomy().unwrap_or_default();
		for (name, slug) in unknown {
			let matched = taxonomy
				.iter()
				.find(|t| t.slug == slug || t.name.eq_ignore_ascii_case(&name));
			match matched.map(|t| (t.group.as_str(), t.slug.as_str())) {
				Some(("genre", found)) => search.genres.include.push(found.into()),
				Some(("theme", found)) => search.themes.include.push(found.into()),
				Some((_, found)) => search.tags.include.push(found.into()),
				None => search.tags.include.push(slug),
			}
		}
	}
}

impl DynamicFilters for OniSaga {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		let (ids, options) = self.genres()?;
		Ok(vec![
			MultiSelectFilter {
				id: "genre".into(),
				title: Some("Genres".into()),
				is_genre: true,
				can_exclude: true,
				uses_tag_style: true,
				options: options.into_iter().map(Into::into).collect(),
				ids: Some(ids.into_iter().map(Into::into).collect()),
				..Default::default()
			}
			.into(),
		])
	}
}

impl DynamicSettings for OniSaga {
	fn get_dynamic_settings(&self) -> Result<Vec<Setting>> {
		let (values, titles) = self.genres()?;
		Ok(vec![
			GroupSetting {
				title: "Exclusions".into(),
				footer: Some(
					"Titles with these genres are hidden unless a search includes them (up to 20)."
						.into(),
				),
				items: vec![
					MultiSelectSetting {
						key: "excludedGenres".into(),
						title: "Excluded Genres".into(),
						values: values.into_iter().map(Into::into).collect(),
						titles: Some(titles.into_iter().map(Into::into).collect()),
						refreshes: Some(vec!["content".into(), "listings".into()]),
						..Default::default()
					}
					.into(),
				],
				..Default::default()
			}
			.into(),
		])
	}
}
