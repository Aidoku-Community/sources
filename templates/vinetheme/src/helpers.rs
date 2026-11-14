use aidoku::{
	Manga, Result,
	alloc::{String, Vec},
	helpers::uri::decode_uri,
	imports::{
		html::Html,
		net::{Request, Response},
	},
	prelude::*,
};

pub fn plain_text(value: &str) -> Option<String> {
	Html::parse(format!("<div id=plain>{value}</div>"))
		.ok()?
		.select_first("#plain")?
		.text()
		.filter(|text| !text.trim().is_empty())
}

pub fn resolve_urls(manga: &mut Manga, base_url: &str) {
	for url in [&mut manga.cover, &mut manga.url].into_iter().flatten() {
		*url = absolute_url(base_url, url);
	}
}

pub fn response(request: Request) -> Result<Response> {
	let response = request.send()?;
	let status = response.status_code();
	if !(200..300).contains(&status) {
		let url = response.get_url().unwrap_or_default();
		let content_type = response.get_header("Content-Type");
		let prefix = response
			.get_string()
			.ok()
			.map(|body| body.chars().take(240).collect::<String>());
		println!(
			"URL: {url}; HTTP {status}; Content-Type: {content_type:?}; body prefix: {prefix:?}"
		);
		bail!("HTTP {status}");
	}
	Ok(response)
}

pub fn absolute_url(base_url: &str, path: &str) -> String {
	if path.starts_with("https://") || path.starts_with("http://") {
		path.into()
	} else if path.starts_with("//") {
		format!("https:{path}")
	} else {
		format!(
			"{}/{}",
			base_url.trim_end_matches('/'),
			path.trim_start_matches('/')
		)
	}
}

pub fn series_path(key: &str) -> String {
	if key.starts_with("novel/") || key.starts_with("comic/") {
		key.into()
	} else {
		format!("comic/{key}")
	}
}

pub fn link_keys(url: &str) -> Option<(String, Option<String>)> {
	let rest = url.split_once("/series/")?.1.split(['?', '#']).next()?;
	let mut parts = rest.trim_matches('/').split('/');
	let kind = parts.next()?;
	if !matches!(kind, "comic" | "novel") {
		return None;
	}
	let slug = parts.next().filter(|s| !s.is_empty())?;
	let key = if kind == "novel" {
		format!("novel/{slug}")
	} else {
		slug.into()
	};
	let chapter = match parts.next() {
		None => None,
		Some("chapter") => Some(decode_uri(parts.next().filter(|s| !s.is_empty())?)),
		_ => return None,
	};
	if parts.next().is_some() {
		return None;
	}
	Some((key, chapter))
}

pub fn title_case(value: &str) -> String {
	let mut output = String::with_capacity(value.len());
	let mut capitalize = true;
	for character in value.replace('_', " ").chars() {
		if capitalize && character.is_alphabetic() {
			output.extend(character.to_uppercase());
			capitalize = false;
		} else {
			output.push(character);
			capitalize = character == ' ';
		}
	}
	output
}

pub fn credit_names(value: Option<String>) -> Option<Vec<String>> {
	let mut names = Vec::new();
	for name in value
		.as_deref()?
		.split([',', '&'])
		.map(str::trim)
		.filter(|name| !name.is_empty())
	{
		if !names.iter().any(|existing| existing == name) {
			names.push(name.into());
		}
	}
	(!names.is_empty()).then_some(names)
}
