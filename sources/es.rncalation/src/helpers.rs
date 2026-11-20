use aidoku::{
	Listing, Result,
	alloc::{String, Vec},
	helpers::uri::encode_uri_component,
	imports::{
		html::Document,
		net::{Request, Response},
	},
	prelude::*,
};

use crate::settings::is_adult;

pub const BASE_URL: &str = "https://rncalation.online";

/// Splits an absolute or relative url into its path segments.
pub fn path_segments(url: &str) -> Vec<&str> {
	let url = url.split(['?', '#']).next().unwrap_or_default();
	let path = match url.split_once("://") {
		Some((_, rest)) => rest
			.split_once('/')
			.map(|(_, path)| path)
			.unwrap_or_default(),
		None => url,
	};
	path.split('/').filter(|part| !part.is_empty()).collect()
}

/// Manga keys are the series slug, from `/comics/{slug}`.
pub fn manga_key(url: &str) -> Option<String> {
	match path_segments(url).as_slice() {
		// `/comics/random` is a "random series" button, not a series.
		["comics", slug] if *slug != "random" => Some((*slug).into()),
		_ => None,
	}
}

pub fn manga_url(key: &str) -> String {
	format!("{BASE_URL}/comics/{key}")
}

/// Chapter keys are the reader code, from `/leer/{code}`.
pub fn chapter_key(url: &str) -> Option<String> {
	match path_segments(url).as_slice() {
		["leer", code] => Some((*code).into()),
		_ => None,
	}
}

pub fn chapter_url(key: &str) -> String {
	format!("{BASE_URL}/leer/{key}")
}

pub fn popular_listing() -> Listing {
	Listing {
		id: "Popular".into(),
		name: "Más vistos".into(),
		..Default::default()
	}
}

pub fn latest_listing() -> Listing {
	Listing {
		id: "Latest".into(),
		name: "Últimas actualizaciones".into(),
		..Default::default()
	}
}

/// Query parameters of `/library`. Empty values are left out.
pub struct LibraryParams<'a> {
	pub query: Option<&'a str>,
	pub sort: &'a str,
	pub kind: &'a str,
	pub status: &'a str,
	pub genre: &'a str,
}

impl<'a> LibraryParams<'a> {
	pub fn sorted(sort: &'a str) -> Self {
		Self {
			query: None,
			sort,
			kind: "",
			status: "",
			genre: "",
		}
	}
}

pub fn library_url(params: &LibraryParams, page: i32) -> String {
	let mut url = format!("{BASE_URL}/library?sort={}", params.sort);
	let mut push = |name: &str, value: &str| {
		if !value.is_empty() {
			url.push_str(&format!("&{name}={}", encode_uri_component(value)));
		}
	};
	push("q", params.query.unwrap_or_default());
	push("type", params.kind);
	push("status", params.status);
	push("genre", params.genre);
	// The first page redirects to the url without `page`.
	if page > 1 {
		url.push_str(&format!("&page={page}"));
	}
	url
}

/// Fetches a page of the site.
///
/// Without the `mv_ck` cookie the site answers with a small page that sets it
/// and reloads with JavaScript. The cookie is saved by then, so retrying once
/// gets the real page.
pub fn fetch(url: &str) -> Result<(Response, Document)> {
	let response = request(url, None)?.send()?;
	let html = response.get_html()?;
	if html.select_first("#mv-verifying").is_none() {
		return Ok((response, html));
	}

	// Also send the cookie ourselves, in case it wasn't stored.
	let cookie = response
		.get_header("Set-Cookie")
		.and_then(|header| header.split(';').next().map(String::from))
		.filter(|cookie| cookie.starts_with("mv_ck="));
	let response = request(url, cookie.as_deref())?.send()?;
	let html = response.get_html()?;
	if html.select_first("#mv-verifying").is_some() {
		bail!("No se pudo pasar la verificación del sitio");
	}
	Ok((response, html))
}

/// A request with our cookies. The app adds the cookies it has stored to them.
fn request(url: &str, cookie: Option<&str>) -> Result<Request> {
	let mut cookies = Vec::new();
	// The cookie the site sets when you confirm your age, only with the setting on.
	if is_adult() {
		cookies.push("age_verified=1");
	}
	cookies.extend(cookie);

	let mut request = Request::get(url)?;
	if !cookies.is_empty() {
		request.set_header("Cookie", &cookies.join("; "));
	}
	Ok(request)
}

/// Series rated Adulto answer 403 until you confirm your age.
pub fn check_age_gate(response: &Response) -> Result<()> {
	if response.status_code() == 403 && !is_adult() {
		bail!(
			"Contenido +18: activa «Soy mayor de 18 años» en los ajustes de la fuente para ver los capítulos"
		);
	}
	Ok(())
}

pub fn fetch_html(url: &str) -> Result<Document> {
	fetch(url).map(|(_, html)| html)
}
