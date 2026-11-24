use crate::models::FeaturedSeries;
use aidoku::{
	Result,
	alloc::{String, Vec},
	imports::html::Document,
	prelude::*,
};
use serde::de::DeserializeOwned;

fn decode(page: &Document) -> String {
	let mut flight = String::new();
	// A slide record can span several script chunks.
	for data in page
		.select("script:not([src])")
		.into_iter()
		.flatten()
		.filter_map(|script| script.data().or_else(|| script.html()))
	{
		let Some(chunk) = data.trim().strip_prefix("self.__next_f.push(") else {
			continue;
		};
		if let Some(Ok((1, text))) = serde_json::Deserializer::from_str(chunk)
			.into_iter::<(u8, String)>()
			.next()
		{
			flight.push_str(&text);
		}
	}
	flight
}

fn extract_array<T: DeserializeOwned>(body: &str, property: &str) -> Option<Vec<T>> {
	body.match_indices(property).find_map(|(start, _)| {
		serde_json::Deserializer::from_str(&body[start + property.len()..])
			.into_iter::<Vec<T>>()
			.next()?
			.ok()
			.filter(|items| !items.is_empty())
	})
}

pub fn featured_series(page: &Document, property: &str) -> Vec<FeaturedSeries> {
	extract_array(&decode(page), property).unwrap_or_default()
}

pub fn reader_images<T: DeserializeOwned>(body: &str) -> Result<Vec<T>> {
	// The first "images" array that reads as pages; another can come first.
	extract_array(body, r#""images":"#).ok_or_else(|| error!("This chapter has missing pages"))
}
