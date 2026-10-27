use aidoku::{
	Result,
	alloc::{String, Vec},
	imports::net::Request,
	prelude::*,
};

pub const BASE_URL: &str = "https://manga-oni.com";

/// The first path segment of a series url. A series only loads under its own type.
const SERIES_TYPES: [&str; 5] = ["manga", "manhwa", "manhua", "oneshot", "novela"];

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

/// Manga keys are the series path, e.g. `/manga/one-piece/`, since the type is
/// part of the url.
pub fn manga_key(url: &str) -> Option<String> {
	match path_segments(url).as_slice() {
		[kind, slug, ..] if SERIES_TYPES.contains(kind) => Some(format!("/{kind}/{slug}/")),
		_ => None,
	}
}

/// Chapter keys are the chapter id, from `/lector/{slug}/{id}/` (or `/leer/` for
/// novels). Some links end in `/cascada/` and some don't, so the id is the stable part.
pub fn chapter_key(url: &str) -> Option<String> {
	match path_segments(url).as_slice() {
		["lector" | "leer", _, id, ..] if id.bytes().all(|b| b.is_ascii_digit()) => {
			Some((*id).into())
		}
		_ => None,
	}
}

/// The reader page of a chapter. Novels use `/leer/`, everything else `/lector/`.
pub fn chapter_url(manga_key: &str, chapter_key: &str) -> String {
	let (reader, slug) = match path_segments(manga_key).as_slice() {
		["novela", slug, ..] => ("leer", *slug),
		[_, slug, ..] => ("lector", *slug),
		_ => ("lector", ""),
	};
	format!("{BASE_URL}/{reader}/{slug}/{chapter_key}/")
}

pub fn directory_url(params: &DirectoryParams, page: i32) -> String {
	format!(
		"{BASE_URL}/directorio?genero={}&estado={}&filtro={}&tipo={}&adulto={}&orden={}&p={page}",
		params.genre, params.status, params.sort, params.kind, params.adult, params.order
	)
}

/// Query parameters of `/directorio`. `"false"` means "any".
pub struct DirectoryParams<'a> {
	pub genre: &'a str,
	pub status: &'a str,
	pub sort: &'a str,
	pub kind: &'a str,
	pub adult: &'a str,
	pub order: &'a str,
}

impl DirectoryParams<'_> {
	/// Most visited first, the same as the site's directory.
	pub fn popular(hide_nsfw: bool) -> Self {
		Self {
			genre: "false",
			status: "false",
			sort: "visitas",
			kind: "false",
			adult: if hide_nsfw { "0" } else { "false" },
			order: "desc",
		}
	}
}

/// For chapter deep links: the reader page links back to its series.
pub fn manga_key_from_reader(url: &str) -> Result<String> {
	Request::get(url)?
		.html()?
		.select_first("h1 a.ajp[href]")
		.and_then(|link| manga_key(&link.attr("abs:href")?))
		.ok_or_else(|| error!("Serie no encontrada"))
}
