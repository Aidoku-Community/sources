use aidoku::{
	MangaStatus, Result,
	alloc::{String, Vec},
	imports::net::Request,
	prelude::*,
};
use alloc::string::ToString;
use serde::{Deserialize, de::DeserializeOwned};

use crate::graphql::GraphQLQuery;

const GRAPHQL_URL: &str = "https://admin.hq-now.com/graphql";

#[derive(Deserialize)]
pub struct GqlResponse<T> {
	pub data: Option<T>,
}

pub fn execute_query<T: DeserializeOwned>(
	gql: &GraphQLQuery,
	variables: Option<serde_json::Value>,
) -> Result<T> {
	let mut body = serde_json::json!({
		"operationName": gql.operation_name,
		"query": gql.query,
	});
	if let Some(vars) = variables {
		body["variables"] = vars;
	}
	let body_str = body.to_string();
	let resp = Request::post(GRAPHQL_URL)
		.map_err(|_| error!("network error"))?
		.header("Content-Type", "application/json")
		.body(body_str.as_bytes())
		.string()?;
	let wrapper: GqlResponse<T> = serde_json::from_str(&resp).map_err(|_| error!("parse error"))?;
	wrapper.data.ok_or_else(|| error!("no data"))
}

pub fn paginate<T>(items: Vec<T>, page: i32, per_page: usize) -> (Vec<T>, bool) {
	let start = ((page - 1) as usize) * per_page;
	let has_next = start + per_page < items.len();
	let slice = items.into_iter().skip(start).take(per_page).collect();
	(slice, has_next)
}

pub fn parse_status(s: Option<&str>) -> MangaStatus {
	match s {
		Some("Concluído") => MangaStatus::Completed,
		Some("Em Andamento") => MangaStatus::Ongoing,
		Some("Cancelado") => MangaStatus::Cancelled,
		Some("Hiatus") => MangaStatus::Hiatus,
		_ => MangaStatus::Unknown,
	}
}

/// Rewrites `http://` image URLs to `https://` to prevent mixed-content blocks.
pub fn to_https(url: Option<String>) -> Option<String> {
	url.map(|u| {
		if u.starts_with("http://") {
			u.replacen("http://", "https://", 1)
		} else {
			u
		}
	})
}
