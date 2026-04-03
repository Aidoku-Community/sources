use aidoku::{
	Result,
	alloc::{String, Vec, string::ToString},
	helpers::uri::encode_uri_component,
	imports::{defaults::defaults_get, net::Request},
	prelude::format,
};
use core::fmt::Write;

pub fn format_names(raw: &str) -> Option<String> {
	if raw.is_empty() {
		return None;
	}
	let formatted: String = raw
		.split(' ')
		.filter(|s| !s.is_empty())
		.map(|segment| {
			segment
				.split('-')
				.map(|word| {
					let mut chars = word.chars();
					match chars.next() {
						Some(c) => {
							let upper: String = c.to_uppercase().collect();
							let rest: String = chars.collect();
							let mut out = upper;
							out.push_str(&rest);
							out
						}
						None => String::new(),
					}
				})
				.collect::<Vec<String>>()
				.join("-")
		})
		.collect::<Vec<String>>()
		.join(", ");
	Some(formatted)
}

pub fn split_tags(raw: &str) -> Vec<String> {
	if raw.is_empty() {
		return Vec::new();
	}
	raw.split(' ')
		.filter(|s| !s.is_empty())
		.map(|s| s.trim().into())
		.collect()
}

pub fn get_adult_mode() -> String {
	let selected = defaults_get::<Vec<String>>("adult_mode").unwrap_or_default();
	let has_sfw = selected.iter().any(|s| s == "false");
	let has_nsfw = selected.iter().any(|s| s == "true");
	match (has_sfw, has_nsfw) {
		(true, true) => "both".to_string(),
		(false, true) => "true".to_string(),
		_ => "false".to_string(),
	}
}

pub fn extract_manga_id(url: &str) -> Option<String> {
	let marker = "/manga/";
	let (_, tail) = url.split_once(marker)?;
	let id = tail
		.split(['/', '?', '#'])
		.next()
		.filter(|s| !s.is_empty())?;
	Some(id.into())
}

pub fn extract_reader_path(url: &str) -> Option<String> {
	let marker = "/reader/";
	let (_, tail) = url.split_once(marker)?;
	let path = tail.split(['?', '#']).next().filter(|s| !s.is_empty())?;
	Some(path.into())
}

pub fn get_server_domain() -> String {
	defaults_get::<String>("server")
		.filter(|s| !s.is_empty())
		.unwrap_or_else(|| "noy.asia".to_string())
}

pub fn get_base_url() -> String {
	format!("https://api.{}", get_server_domain())
}

pub fn get_img_base() -> String {
	format!("https://img.{}", get_server_domain())
}

pub fn build_form_body(params: &[(&str, &str)]) -> String {
	let mut out = String::new();
	for (k, v) in params {
		if v.is_empty() {
			continue;
		}
		if !out.is_empty() {
			out.push('&');
		}
		let _ = write!(out, "{k}={}", encode_uri_component(v));
	}
	out
}

pub fn post_with_form(url: &str, body: &str, referer: &str, adult: &str) -> Result<Request> {
	Ok(Request::post(url)?
		.header("Content-Type", "application/x-www-form-urlencoded")
		.header("Referer", referer)
		.header("allow-adult", adult)
		.body(body.as_bytes()))
}
