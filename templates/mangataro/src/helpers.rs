use crate::{models::Slug, settings};
use aidoku::{
	ContentRating, MangaStatus, Result, Viewer,
	alloc::{String, Vec, borrow::Cow, collections::BTreeSet},
	helpers::uri::{QueryParameters, decode_uri},
	imports::{
		html::{Document, Html},
		net::{Request, Response},
		std::{current_date, parse_date_with_options},
	},
	prelude::*,
};
use serde::de::DeserializeOwned;

pub fn request(base_url: &str, url: &str) -> Result<Request> {
	Ok(site_headers(Request::get(url)?, base_url))
}

pub fn site_headers(request: Request, base_url: &str) -> Request {
	request
		.header("Referer", base_url)
		.header("Accept", "application/json,text/html,*/*")
		.header("X-Requested-With", "XMLHttpRequest")
}

pub fn checked(response: Response) -> Result<Response> {
	let status = response.status_code();
	if !(200..300).contains(&status) {
		bail!("The site returned status {status}");
	}
	Ok(response)
}

pub fn parse<T: DeserializeOwned>(response: Response) -> Result<T> {
	checked(response)?.get_json_owned()
}

pub fn html(base_url: &str, url: &str) -> Result<Document> {
	Ok(checked(request(base_url, url)?.send()?)?.get_html()?)
}

// The browse page lists every type, status, year and tag with its id
pub fn browse(base_url: &str) -> Result<Document> {
	html(base_url, &format!("{base_url}/browse"))
}

// Browse takes tag and year ids as numbers
pub fn number_list(ids: &[String]) -> Result<String> {
	let ids = ids
		.iter()
		.map(|id| id.parse::<i64>())
		.collect::<core::result::Result<Vec<_>, _>>()
		.map_err(|_| error!("Invalid filter id"))?;
	Ok(serde_json::to_string(&ids)?)
}

pub fn novel_title(mut title: String) -> String {
	if settings::label_novels() && !title.ends_with("(Novel)") {
		title.push_str(" (Novel)");
	}
	title
}

pub fn plain_text(value: &str) -> Result<String> {
	Html::parse(format!("<div id=plain>{value}</div>"))?
		.select_first("#plain")
		.and_then(|el| el.text())
		.ok_or(error!("Invalid text"))
}

pub fn series_key(url: &str) -> Option<&str> {
	let path = url.split(['?', '#']).next()?;
	let rest = path
		.split("/manga/")
		.nth(1)
		.or_else(|| path.split("/read/").nth(1))?;
	let key = rest.split('/').next()?;
	(!key.is_empty()).then_some(key)
}

pub fn date(value: &str) -> Option<i64> {
	let text = value.trim().to_ascii_lowercase();
	if text == "just now" || text == "now" {
		return Some(current_date());
	}
	let number = text
		.find(|c: char| !c.is_ascii_digit())
		.unwrap_or(text.len());
	if let Ok(amount) = text[..number].parse::<i64>() {
		let unit = text[number..].trim_start();
		let seconds = if unit.starts_with("mo") {
			2_592_000
		} else if unit.starts_with('y') {
			31_536_000
		} else if unit.starts_with('w') {
			604_800
		} else if unit.starts_with('d') {
			86_400
		} else if unit.starts_with('h') {
			3_600
		} else if unit.starts_with('m') {
			60
		} else if unit.starts_with('s') {
			1
		} else {
			0
		};
		if seconds > 0 {
			return Some(current_date() - amount * seconds);
		}
	}
	parse_date_with_options(
		format!("{value} 00:00"),
		"MMM d, yyyy HH:mm",
		"en_US_POSIX",
		"UTC",
	)
}

pub fn tags(document: &Document) -> Vec<String> {
	let mut tags = document
		.select(".manga-page-wrapper a[href*='/tag/']")
		.into_iter()
		.flatten()
		.filter_map(|el| el.text())
		.collect::<Vec<_>>();
	tags.sort_unstable();
	tags.dedup();
	tags
}

pub fn rating(tags: &[String]) -> ContentRating {
	if tags.iter().any(|tag| {
		matches!(
			tag.to_ascii_lowercase().as_str(),
			"adult" | "hentai" | "erotica" | "smut" | "pornographic" | "19+"
		)
	}) {
		ContentRating::NSFW
	} else if tags.iter().any(|tag| {
		matches!(
			tag.to_ascii_lowercase().as_str(),
			"mature" | "ecchi" | "suggestive"
		)
	}) {
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	}
}

pub fn status(value: &str) -> MangaStatus {
	match value.to_ascii_lowercase().as_str() {
		"ongoing" => MangaStatus::Ongoing,
		"completed" => MangaStatus::Completed,
		"cancelled" => MangaStatus::Cancelled,
		"hiatus" => MangaStatus::Hiatus,
		_ => MangaStatus::Unknown,
	}
}

pub fn viewer(kind: &str) -> Viewer {
	match kind {
		"Novel" => Viewer::Unknown,
		"Manhwa" | "Manhua" | "Webtoon" => Viewer::Webtoon,
		_ => Viewer::RightToLeft,
	}
}

pub fn link_keys(url: &str) -> Option<(String, Option<String>)> {
	let key = series_key(url)?;
	let chapter = url.split("/read/").nth(1).and_then(|path| {
		let path = path.split(['?', '#']).next()?.trim_end_matches('/');
		let (_, chapter) = path.split_once('/')?;
		chapter.rsplit_once('-')?.1.parse::<u64>().ok()?;
		Some(path.into())
	});
	Some((decode_uri(key), chapter))
}

pub fn excluded_slugs<'a>(
	base_url: &str,
	keys: impl Iterator<Item = &'a str>,
	excluded: &[String],
) -> Result<BTreeSet<String>> {
	if excluded.is_empty() {
		return Ok(BTreeSet::new());
	}
	let keys = keys
		.collect::<BTreeSet<_>>()
		.into_iter()
		.collect::<Vec<_>>();
	let mut requests = Vec::new();
	for chunk in keys.chunks(100) {
		let mut query = QueryParameters::new();
		query.push("slug", Some(&chunk.join(",")));
		query.push_encoded("tags", Some(&excluded.join(",")));
		query.push_encoded("per_page", Some("100"));
		query.push_encoded("_fields", Some("slug"));
		requests.push(request(
			base_url,
			&format!("{base_url}/wp-json/wp/v2/manga?{query}"),
		)?);
	}
	let mut blocked = BTreeSet::new();
	for response in Request::send_all(requests) {
		blocked.extend(parse::<Vec<Slug>>(response?)?.into_iter().map(|x| x.slug));
	}
	Ok(blocked)
}

pub fn options(
	document: &Document,
	class: &str,
) -> (Vec<Cow<'static, str>>, Vec<Cow<'static, str>>) {
	document
		.select(format!(".{class}[data-value]"))
		.into_iter()
		.flatten()
		.filter_map(|el| {
			let id = el.attr("data-value").filter(|s| !s.is_empty())?;
			let name = el.text()?;
			let name = if class == "year-btn" {
				name
			} else {
				name.trim_end_matches(|c: char| c.is_ascii_digit())
					.trim_end()
					.into()
			};
			Some((id.into(), name.into()))
		})
		.unzip()
}
