use crate::models::NextData;
use aidoku::{
	Listing, Result, alloc::string::ToString, helpers::uri::QueryParameters, imports::net::Request,
	prelude::*,
};
use serde::de::DeserializeOwned;

pub const SORTS: [&str; 8] = [
	"latest",
	"popular",
	"newest",
	"rating",
	"views_today",
	"views_7days",
	"views_30days",
	"views",
];

pub fn next_data<T: DeserializeOwned>(url: &str) -> Result<T> {
	let html = Request::get(url)?.html()?;
	let script = html
		.select_first("script#__NEXT_DATA__")
		.ok_or_else(|| error!("Missing __NEXT_DATA__"))?;
	let json = script
		.data()
		.or_else(|| script.html())
		.ok_or_else(|| error!("Empty __NEXT_DATA__"))?;
	Ok(serde_json::from_str::<NextData<T>>(&json)?.props.page_props)
}

pub fn listing_query(id: &str) -> QueryParameters {
	let mut qs = QueryParameters::new();
	match id {
		"today" | "week" => {
			qs.push("sort", Some("popular"));
			qs.push("window", Some(id));
		}
		"uncensored" | "raw" => qs.push(&format!("is_{id}"), Some("true")),
		_ => qs.push("sort", Some("latest")),
	}
	qs
}

pub fn listing(id: &str, name: &str) -> Option<Listing> {
	Some(Listing {
		id: id.to_string(),
		name: name.to_string(),
		..Default::default()
	})
}
