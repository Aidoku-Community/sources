use crate::BASE_URL;
use crate::models::{ApiEntity, ApiTagsResponse};
use aidoku::{
	Result,
	alloc::{
		string::{String, ToString},
		vec::Vec,
	},
	helpers::uri::encode_uri_component,
	imports::net::Request,
	prelude::*,
};

pub fn find_tag_id(keyword: &str, tag_type: &str) -> Result<Option<String>> {
	let response = Request::get(format!(
		"{BASE_URL}/api/tags?keyword={}",
		encode_uri_component(keyword)
	))?
	.header("Accept", "application/json")
	.header("Referer", &format!("{BASE_URL}/browse"))
	.send()?
	.get_json::<ApiTagsResponse>()?;

	Ok(response
		.data
		.iter()
		.find(|tag| tag.tag_type == tag_type && tag.name.eq_ignore_ascii_case(keyword))
		.or_else(|| response.data.iter().find(|tag| tag.tag_type == tag_type))
		.map(|tag| tag.id.to_string()))
}

pub fn entity_titles(entities: Vec<ApiEntity>) -> Vec<String> {
	entities.into_iter().map(|entity| entity.title).collect()
}

pub fn api_tags(genres: Option<Vec<ApiEntity>>, themes: Option<Vec<ApiEntity>>) -> Vec<String> {
	let mut tags = Vec::new();
	if let Some(genres) = genres {
		tags.extend(entity_titles(genres));
	}
	if let Some(themes) = themes {
		tags.extend(entity_titles(themes));
	}
	tags
}
