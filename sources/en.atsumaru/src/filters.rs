use crate::{
	Atsumaru,
	api::{API_URL, get},
	helpers::allows,
	models::{AvailableFilters, TagOption},
	settings,
};
use aidoku::{
	DynamicSettings, GroupSetting, MultiSelectSetting, PageSetting, Result, Setting,
	alloc::{String, Vec, format, vec},
};

pub const GROUPS: [&str; 19] = [
	"Activities",
	"Audience Demographics",
	"Character Archetype",
	"Character Traits",
	"Character Types",
	"Derivative Work",
	"Locations",
	"Narrative Tropes",
	"Objects",
	"Occupations",
	"Relationship",
	"Settings",
	"Sexual Content",
	"Species & Creatures",
	"Themes",
	"Time Period",
	"Tone",
	"Work Info",
	"World Building",
];

/// Explicit tags are offered only when pornographic titles are, as the site does.
fn grouped_tags(tags: Vec<TagOption>) -> Vec<(&'static str, Vec<String>, Vec<String>)> {
	let explicit = settings::adult_only() || allows(&settings::content_ratings(), "Pornographic");
	let mut tags = tags
		.into_iter()
		.filter(|tag| explicit || !tag.adult)
		.filter_map(|tag| Some((GROUPS.iter().position(|group| *group == tag.group)?, tag)))
		.collect::<Vec<_>>();
	tags.sort_by(|(a, a_tag), (b, b_tag)| a.cmp(b).then_with(|| a_tag.name.cmp(&b_tag.name)));
	let mut groups: Vec<(&'static str, Vec<String>, Vec<String>)> = Vec::new();
	for (index, tag) in tags {
		let group = GROUPS[index];
		match groups.last_mut() {
			Some((current, ids, names)) if *current == group => {
				ids.push(tag.id);
				names.push(tag.name);
			}
			_ => groups.push((group, vec![tag.id], vec![tag.name])),
		}
	}
	groups
}

/// The site keeps adding tags, so its genre and tag lists are fetched rather than kept here.
fn available_filters() -> Result<AvailableFilters> {
	get(&format!("{API_URL}/explore/availableFilters"))
}

/// A tapped tag arrives by name, but search only matches tags by id.
pub fn tag_ids(names: &[String]) -> Result<(Vec<String>, Vec<String>)> {
	let AvailableFilters { genres, tags } = available_filters()?;
	let wanted = |name: &str| names.iter().any(|wanted| wanted.eq_ignore_ascii_case(name));
	Ok((
		genres
			.into_iter()
			.filter(|genre| wanted(&genre.name))
			.map(|genre| genre.id)
			.collect(),
		tags.into_iter()
			.filter(|tag| wanted(&tag.name))
			.map(|tag| tag.id)
			.collect(),
	))
}

impl DynamicSettings for Atsumaru {
	fn get_dynamic_settings(&self) -> Result<Vec<Setting>> {
		let AvailableFilters { genres, tags } = available_filters()?;
		let refreshes = || Some(vec!["content".into(), "listings".into()]);
		let (genre_ids, genre_names): (Vec<String>, Vec<String>) = genres
			.into_iter()
			.map(|genre| (genre.id, genre.name))
			.unzip();
		let tag_lists = grouped_tags(tags)
			.into_iter()
			.map(|(group, ids, names)| {
				MultiSelectSetting {
					key: settings::hidden_tags_key(group).into(),
					title: group.into(),
					values: ids.into_iter().map(Into::into).collect(),
					titles: Some(names.into_iter().map(Into::into).collect()),
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
					"Titles with any of these are left out of the home page, listings and search, \
					 unless a search asks for the genre or tag."
						.into(),
				),
				items: vec![
					MultiSelectSetting {
						key: "hiddenGenres".into(),
						title: "Hidden Genres".into(),
						values: genre_ids.into_iter().map(Into::into).collect(),
						titles: Some(genre_names.into_iter().map(Into::into).collect()),
						refreshes: refreshes(),
						..Default::default()
					}
					.into(),
					PageSetting {
						key: "hiddenTagsPage".into(),
						title: "Hidden Tags".into(),
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
