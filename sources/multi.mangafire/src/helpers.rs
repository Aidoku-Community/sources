use crate::{
	BASE_URL,
	models::{ApiEntity, ApiTagsResponse},
	vrf,
};
use aidoku::{
	Result,
	alloc::{
		borrow::Cow,
		string::{String, ToString},
		vec::Vec,
	},
	helpers::uri::encode_uri_component,
	imports::net::Request,
	prelude::*,
};

pub fn api_request<'a>(path: &str, params: &mut [(Cow<'a, str>, Cow<'a, str>)]) -> Result<Request> {
	params.sort_by(|left, right| left.0.cmp(&right.0));

	let mut canonical = String::from(path);

	if !params.is_empty() {
		canonical.push('?');

		let mut last_key = "";
		let mut index = 0;

		for (position, (key, value)) in params.iter().enumerate() {
			if position != 0 {
				canonical.push('&');
			}

			if let Some(base) = key.strip_suffix("[]") {
				if last_key != *key {
					index = 0;
				}
				last_key = key;

				canonical.push_str(base);
				canonical.push('[');
				canonical.push_str(&index.to_string());
				canonical.push(']');
				index += 1;
			} else {
				canonical.push_str(key);
			}

			canonical.push('=');
			canonical.push_str(value);
		}
	}

	let mut url = format!("{BASE_URL}/api{path}?");
	for (position, (key, value)) in params.iter().enumerate() {
		if position != 0 {
			url.push('&');
		}

		url.push_str(&encode_uri_component(key.as_ref()));
		url.push('=');
		url.push_str(&encode_uri_component(value.as_ref()));
	}
	if !params.is_empty() {
		url.push('&');
	}
	url.push_str("vrf=");
	url.push_str(&vrf::sign(&canonical));

	Ok(Request::get(url)?)
}

pub fn find_tag_id(keyword: &str, tag_type: &str) -> Result<Option<String>> {
	let response = api_request("/tags", &mut [("keyword".into(), keyword.into())])?
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
