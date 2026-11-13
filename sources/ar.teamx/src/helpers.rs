use crate::BASE_URL;
use aidoku::{
	Chapter, Manga,
	alloc::{String, format, string::ToString},
	imports::html::Element,
};

/// Turns any site URL into the manga key (`/series/<slug>`).
pub fn manga_path(url: &str) -> Option<String> {
	let path = url.strip_prefix(BASE_URL)?;
	let mut segments = path.trim_start_matches('/').split('/');
	if segments.next()? != "series" {
		return None;
	}
	let slug = segments.next().filter(|slug| !slug.is_empty())?;
	Some(format!("/series/{slug}"))
}

/// Turns a chapter URL into its manga key and chapter key.
pub fn chapter_path(url: &str) -> Option<(String, String)> {
	let path = url.strip_prefix(BASE_URL)?;
	let mut segments = path.trim_start_matches('/').split('/');
	if segments.next()? != "series" {
		return None;
	}
	let slug = segments.next().filter(|slug| !slug.is_empty())?;
	let chapter = segments.next().filter(|chapter| !chapter.is_empty())?;
	Some((
		format!("/series/{slug}"),
		format!("/series/{slug}/{chapter}"),
	))
}

pub fn parse_card(el: &Element) -> Option<Manga> {
	let link = el.select_first("a")?;
	let href = link.attr("abs:href")?;
	let img = el.select_first("img");
	let cover = img
		.as_ref()
		.and_then(|img| img.attr("abs:data-src").filter(|src| !src.is_empty()))
		.or_else(|| img.and_then(|img| img.attr("abs:src")));
	Some(Manga {
		key: manga_path(&href)?,
		title: link.attr("title")?,
		cover,
		url: Some(href),
		..Default::default()
	})
}

pub fn parse_chapter(el: &Element) -> Option<Chapter> {
	if el.select_first("span.locked").is_some() {
		return None;
	}
	let link = el.select_first("a")?;
	let href = link.attr("abs:href")?;
	let number = el.attr("data-number").and_then(|value| value.parse().ok());
	let extra_title = el
		.select_first("div.chapter-info div.chapter-title")
		.and_then(|el| el.text())
		.filter(|title| !title.trim().is_empty());

	let mut title = match number {
		Some(number) => format!("الفصل {number}"),
		None => String::new(),
	};
	if let Some(extra_title) = extra_title {
		let number = el.attr("data-number").unwrap_or_default();
		let generic = [
			number.clone(),
			format!("الفصل {number}"),
			format!("الفصل رقم {number}"),
		];
		if !generic.iter().any(|generic| generic == extra_title.trim()) {
			title.push_str(" - ");
			title.push_str(extra_title.trim());
		}
	}
	title.push('\u{200F}');

	Some(Chapter {
		key: href.strip_prefix(BASE_URL)?.to_string(),
		title: Some(title),
		chapter_number: number,
		date_uploaded: el
			.attr("data-date")
			.and_then(|value| value.parse::<i64>().ok())
			.map(|seconds| seconds * 1000),
		url: Some(href),
		..Default::default()
	})
}
