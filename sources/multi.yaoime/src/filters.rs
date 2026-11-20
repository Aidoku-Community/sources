use crate::{
	BASE_URL, YaoiMe,
	helpers::{page_data, send},
	models::{FilterLists, Named, SiteSettings},
	tags::{self, GROUPS},
};
use aidoku::{
	DynamicFilters, DynamicSettings, Filter, GroupSetting, MultiSelectFilter, MultiSelectSetting,
	PageSetting, RangeFilter, Result, SelectFilter, Setting, TextFilter,
	alloc::{String, Vec, format, vec},
};

// The browse page lists every genre and tag the site has, most used first, and its years.
fn filter_lists() -> Result<FilterLists> {
	let body = send(&format!("{BASE_URL}/browse"), &SiteSettings::current())?.get_string()?;
	page_data(&body, "{\"languages\":[")
}

fn grouped(tags: Vec<Named>) -> Vec<(&'static str, &'static str, Vec<String>)> {
	let mut groups: Vec<_> = GROUPS
		.iter()
		.map(|(key, title)| (*key, *title, Vec::new()))
		.collect();
	for tag in tags {
		let key = tags::group(&tag.name);
		if let Some((.., names)) = groups.iter_mut().find(|(group, ..)| *group == key) {
			names.push(tag.name);
		}
	}
	groups.retain(|(.., names)| !names.is_empty());
	groups
}

// Built here rather than in filters.json so they keep the site's order after the static ones.
impl DynamicFilters for YaoiMe {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		let FilterLists {
			genres,
			tags,
			years,
		} = filter_lists()?;
		let mut filters: Vec<Filter> = vec![
			MultiSelectFilter {
				id: "genre".into(),
				title: Some("Genres".into()),
				is_genre: true,
				can_exclude: true,
				uses_tag_style: true,
				options: genres.into_iter().map(|genre| genre.name.into()).collect(),
				..Default::default()
			}
			.into(),
		];
		filters.extend(grouped(tags).into_iter().map(|(key, title, names)| {
			MultiSelectFilter {
				id: format!("tags.{key}").into(),
				title: Some(title.into()),
				hide_from_header: Some(true),
				can_exclude: true,
				uses_tag_style: true,
				options: names.into_iter().map(Into::into).collect(),
				..Default::default()
			}
			.into()
		}));
		filters.extend([
			RangeFilter {
				id: "year".into(),
				title: Some("Year".into()),
				hide_from_header: Some(true),
				min: years.first().map(|year| *year as f32),
				max: years.last().map(|year| *year as f32),
				decimal: false,
			}
			.into(),
			RangeFilter {
				id: "chapters".into(),
				title: Some("Chapters".into()),
				hide_from_header: Some(true),
				min: Some(1.0),
				max: None,
				decimal: false,
			}
			.into(),
			RangeFilter {
				id: "rating".into(),
				title: Some("Rating".into()),
				hide_from_header: Some(true),
				min: Some(0.0),
				max: Some(100.0),
				decimal: false,
			}
			.into(),
			TextFilter {
				id: "pages".into(),
				title: Some("Min pages".into()),
				hide_from_header: Some(true),
				placeholder: Some("Pages".into()),
			}
			.into(),
			SelectFilter {
				id: "updated".into(),
				title: Some("Updated within".into()),
				hide_from_header: Some(true),
				options: vec![
					"any time".into(),
					"24 hours".into(),
					"a week".into(),
					"a month".into(),
					"3 months".into(),
					"a year".into(),
				],
				ids: Some(vec![
					"".into(),
					"1".into(),
					"7".into(),
					"30".into(),
					"90".into(),
					"365".into(),
				]),
				..Default::default()
			}
			.into(),
		]);
		Ok(filters)
	}
}

impl DynamicSettings for YaoiMe {
	fn get_dynamic_settings(&self) -> Result<Vec<Setting>> {
		let FilterLists { genres, tags, .. } = filter_lists()?;
		let refreshes = || Some(vec!["content".into(), "listings".into()]);
		let tag_lists = grouped(tags)
			.into_iter()
			.map(|(key, title, names)| {
				MultiSelectSetting {
					key: format!("excludedTags.{key}").into(),
					title: title.into(),
					values: names.into_iter().map(Into::into).collect(),
					refreshes: refreshes(),
					..Default::default()
				}
				.into()
			})
			.collect();
		Ok(vec![
			GroupSetting {
				title: "Exclusions".into(),
				footer: Some(
					"Titles with any of these are hidden, up to 40 in all. Including one in a \
					 search shows it for that search."
						.into(),
				),
				items: vec![
					MultiSelectSetting {
						key: "excludedGenres".into(),
						title: "Excluded Genres".into(),
						values: genres.into_iter().map(|genre| genre.name.into()).collect(),
						refreshes: refreshes(),
						..Default::default()
					}
					.into(),
					PageSetting {
						key: "excludedTagsPage".into(),
						title: "Excluded Tags".into(),
						items: tag_lists,
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
