use crate::models::{DesuItem, DesuResponse};
use crate::settings::domain;
use aidoku::helpers::uri::QueryParameters;
use aidoku::imports::net::Request;
use aidoku::{FilterValue, Result, alloc::String, error};
use alloc::string::ToString;
use alloc::vec::Vec;
use alloc::{format, vec};

pub const PAGE_SIZE: usize = 20;

pub fn get_base_url() -> String {
	format!("https://{}", domain())
}

pub fn get_base_api_url() -> String {
	format!("https://{}/manga/api", domain())
}

pub fn apply_headers(request: Request) -> Request {
	request
		.header("User-Agent", "Aidoku")
		.header("Referer", get_base_url().as_str())
}

pub fn fetch_by_id(id: &str) -> Result<DesuItem> {
	let url = format!("{}/{}", get_base_api_url(), id);
	let response = apply_headers(Request::get(url)?).json_owned::<DesuResponse<DesuItem>>()?;

	if let Some(err) = response.error {
		Err(error!("Failed to fetch \"{}\": {}", id, err))
	} else if let Some(res) = response.response {
		Ok(res)
	} else {
		Err(error!("Failed to fetch \"{}\"", id))
	}
}

pub fn fetch_chapter(item_id: &str, id: &str) -> Result<DesuItem> {
	let url = format!("{}/{}/chapter/{}", get_base_api_url(), item_id, id);
	let response = apply_headers(Request::get(url)?).json_owned::<DesuResponse<DesuItem>>()?;

	if let Some(err) = response.error {
		Err(error!("Failed to fetch \"{}/ch/{}\": {}", item_id, id, err))
	} else if let Some(res) = response.response {
		Ok(res)
	} else {
		Err(error!("Failed to fetch \"{}/ch/{}\"", item_id, id))
	}
}

pub fn search(
	query: Option<String>,
	page: i32,
	filters: Vec<FilterValue>,
) -> Result<Vec<DesuItem>> {
	let mut params = QueryParameters::new();

	params.push("limit", Some(PAGE_SIZE.to_string().as_str()));
	if page > 1 {
		params.push("page", Some(page.to_string().as_str()));
	}

	if let Some(q) = query {
		params.push("search", Some(q.as_str()));
	}

	let mut order = "updated"; // "по обновлению" (idx: 3), default
	let mut genres: Vec<String> = vec![];
	for filter in filters {
		match filter {
			FilterValue::Sort { index, .. } => {
				order = match index {
					0 => "id",
					1 => "name",
					2 => "popular",
					_ => order,
				}
			}
			FilterValue::MultiSelect {
				id,
				included,
				excluded,
			} => {
				let v: Vec<_> = included
					.into_iter()
					.chain(excluded.into_iter().map(|x| format!("!{}", x.as_str())))
					.collect();
				if id.eq("genres") || id.eq("tags") {
					genres.extend(v)
				} else {
					params.push(&id, Some(&v.join(",")))
				}
			}
			FilterValue::Select { id, value } => params.push(&id, Some(&value)),
			_ => continue,
		}
	}
	params.push("order", Some(order));
	if !genres.is_empty() {
		params.push("genres", Some(&genres.join(",")))
	}

	let url = format!("{}/?{}", get_base_api_url(), params);
	let response = apply_headers(Request::get(url)?).json_owned::<DesuResponse<Vec<DesuItem>>>()?;

	if let Some(err) = response.error {
		Err(error!("Failed to run search: {}", err))
	} else if let Some(res) = response.response {
		Ok(res)
	} else {
		Err(error!("Failed to run search: unknown error"))
	}
}
