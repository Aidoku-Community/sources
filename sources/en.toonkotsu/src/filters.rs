use crate::{
	Toonkotsu,
	api::{self, BASE_URL, json_request},
	helpers::is_unlisted_tag,
	models::{Facet, Facets},
};
use aidoku::{
	DynamicFilters, DynamicSettings, Filter, GroupSetting, MultiSelectFilter, MultiSelectSetting,
	Result, Setting,
	alloc::{String, Vec, format, vec},
};

/// The site's tag picker groups, which all filter one parameter.
pub const GROUPS: &[(&str, &str)] = &[
	("genres", "Genres"),
	("themes", "Themes"),
	("content", "Content"),
	("format", "Format"),
];

/// Uncategorised tags the site's picker files under themes rather than genres.
const THEME_TAGS: &[&str] = &[
	"School Life",
	"Martial Arts",
	"Harem",
	"Isekai",
	"Gender Bender",
	"Mecha",
	"Supernatural",
	"Historical",
];

const CONTENT_TAGS: &[&str] = &["Ecchi", "Mature", "Smut", "Adult", "Hentai"];

/// The picker reads every categorised tag as a theme, and leaves the main genres without one.
pub fn group_index(name: &str, category: Option<&str>) -> usize {
	if category.is_some_and(|category| !category.is_empty()) || THEME_TAGS.contains(&name) {
		1
	} else if CONTENT_TAGS.contains(&name) {
		2
	} else if name == "Doujinshi" {
		3
	} else {
		0
	}
}

/// Tags the site offers for browsing, leaving out the explicit ones unless pornographic titles
/// are on, as the site does.
fn listed_facets() -> Result<Vec<Facet>> {
	let sfw = if api::sfw() { 1 } else { 0 };
	let facets: Facets =
		api::parse(json_request(format!("{BASE_URL}/cdn/v1/facets?sfw={sfw}"))?.send()?)?;
	Ok(facets
		.genres
		.unwrap_or_default()
		.into_iter()
		.filter(|facet| !is_unlisted_tag(&facet.name))
		.collect())
}

impl DynamicFilters for Toonkotsu {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		let mut grouped: Vec<Vec<String>> = vec![Vec::new(); GROUPS.len()];

		for facet in listed_facets()? {
			grouped[group_index(&facet.name, facet.category.as_deref())].push(facet.name);
		}

		Ok(GROUPS
			.iter()
			.zip(grouped)
			.filter(|(_, options)| !options.is_empty())
			.map(|((id, title), mut options)| {
				options.sort();
				MultiSelectFilter {
					id: (*id).into(),
					title: Some((*title).into()),
					is_genre: true,
					uses_tag_style: true,
					can_exclude: true,
					options: options.into_iter().map(Into::into).collect(),
					..Default::default()
				}
				.into()
			})
			.collect())
	}
}

/// The tag list lives on the site, so hidden tags are offered from it.
impl DynamicSettings for Toonkotsu {
	fn get_dynamic_settings(&self) -> Result<Vec<Setting>> {
		let mut tags: Vec<String> = listed_facets()?
			.into_iter()
			.map(|facet| facet.name)
			.collect();
		tags.sort();
		Ok(vec![
			GroupSetting {
				title: "Exclusions".into(),
				footer: Some(
					"Titles carrying any of these are left out of Discover, listings and search, \
					 unless a search asks for the tag."
						.into(),
				),
				items: vec![
					MultiSelectSetting {
						key: "hiddenTags".into(),
						title: "Hidden Tags".into(),
						values: tags.into_iter().map(Into::into).collect(),
						refreshes: Some(vec!["content".into()]),
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
