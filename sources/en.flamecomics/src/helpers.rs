use crate::{
	BASE_URL,
	models::{ChapterItem, Entry, NextData, Unlock},
};
use aidoku::{
	Result,
	alloc::{String, Vec, string::ToString},
	helpers::string::PlainText,
	imports::{
		html::{Element, Html, Kind},
		net::Request,
	},
	prelude::*,
};
use serde::de::DeserializeOwned;

/// Every page embeds its data as JSON. The site's data route needs a build id that changes with
/// each deploy and has to be read from another page first.
pub fn next_data<T: DeserializeOwned>(path: &str) -> Result<T> {
	let response = Request::get(format!("{BASE_URL}{path}"))?.send()?;
	if response.status_code() == 404 {
		bail!("Flame Comics has no page at {path}");
	}
	let json = response
		.get_html()?
		.select_first("script#__NEXT_DATA__")
		.and_then(|script| script.data().or_else(|| script.html()))
		.ok_or_else(|| error!("No page data found at {path}"))?;
	Ok(serde_json::from_str::<NextData<T>>(&json)?.props.page_props)
}

/// Novel and series ids overlap, so a key keeps the site's path.
pub fn key(novel_id: Option<u32>, series_id: Option<u32>) -> Option<String> {
	match (novel_id, series_id) {
		(Some(id), _) => Some(format!("/novel/{id}")),
		(None, Some(id)) => Some(format!("/series/{id}")),
		(None, None) => None,
	}
}

/// A search over browse, which lists the whole catalogue; the site filters it locally too.
#[derive(Default)]
pub struct SearchParams {
	pub query: Option<String>,
	pub popular: bool,
	pub descending: bool,
	pub genres: Vec<String>,
	pub hidden_genres: Vec<String>,
	pub types: Vec<String>,
	pub statuses: Vec<String>,
	pub years: Vec<String>,
}

impl SearchParams {
	pub fn matches(&self, entry: &Entry) -> bool {
		let listed = |wanted: &[String], value: Option<&str>| {
			wanted.is_empty()
				|| value.is_some_and(|value| wanted.iter().any(|w| w.eq_ignore_ascii_case(value)))
		};
		self.query
			.as_deref()
			.is_none_or(|query| entry.title.to_lowercase().contains(query))
			&& self.genres.iter().all(|genre| entry.tags.contains(genre))
			&& !self
				.hidden_genres
				.iter()
				.any(|genre| entry.tags.contains(genre))
			&& listed(&self.types, entry.r#type.as_deref())
			&& listed(&self.statuses, entry.status.as_deref())
			&& listed(
				&self.years,
				entry.year.map(|year| year.to_string()).as_deref(),
			)
	}
}

/// An html fragment as Markdown paragraphs, keeping its italics and bold.
pub fn markdown(html: &str) -> String {
	// Some descriptions are bare text with no paragraphs.
	let wrapped;
	let html = if html.contains("<p") {
		html
	} else {
		wrapped = format!("<p>{html}</p>");
		&wrapped
	};
	let Some(paragraphs) = Html::parse_fragment(html)
		.ok()
		.and_then(|document| document.select("p"))
	else {
		return String::new();
	};
	paragraphs
		.map(|paragraph| {
			let mut text = String::new();
			inline(&paragraph, &mut text);
			text.trim().into()
		})
		.filter(|text: &String| !text.is_empty())
		.collect::<Vec<_>>()
		.join("\n\n")
}

// Text nodes come only from `child_nodes` and elements only from `children`, so they are walked
// together. Markers wrap the trimmed text, since `* word *` isn't emphasis.
fn inline(element: &Element, output: &mut String) {
	let mut children = element.children();
	for node in element.child_nodes() {
		match node.kind() {
			Kind::TextNode => output.extend(node.text().map(|text| text.escape_markdown())),
			Kind::Element => {
				let Some(child) = children.next() else {
					continue;
				};
				let mut inner = String::new();
				inline(&child, &mut inner);
				let marker = match child.tag_name().as_deref() {
					Some("strong" | "b") => "**",
					Some("em" | "i") => "*",
					_ => "",
				};
				let text = inner.trim();
				if text.is_empty() {
					output.push_str(&inner);
					continue;
				}
				let start = inner.len() - inner.trim_start().len();
				output.push_str(&inner[..start]);
				output.push_str(marker);
				output.push_str(text);
				output.push_str(marker);
				output.push_str(&inner[start + text.len()..]);
			}
			_ => {}
		}
	}
}

/// The site's own early-access rule: a priced chapter is public once its release time passes,
/// or, under a rolling policy, once it is no longer among the newest few.
pub fn locked_chapters(unlock: &Unlock, chapters: &[ChapterItem], now: i64) -> Vec<bool> {
	let rolling = unlock.unlock_policy.as_deref() == Some("ROLLING_LATEST");
	let mut count = unlock
		.locked_chapter_count
		.filter(|count| *count > 0)
		.unwrap_or(10) as usize;
	if let Some(percent) = unlock.locked_chapter_percent.filter(|p| *p > 0) {
		count = count.max((chapters.len() * percent.min(100) as usize).div_ceil(100));
		if let Some(max) = unlock.locked_chapter_max.filter(|max| *max > 0) {
			count = count.min(max.min(100) as usize);
		}
	}
	let mut numbers: Vec<f32> = chapters.iter().filter_map(ChapterItem::number).collect();
	numbers.sort_by(|a, b| b.total_cmp(a));
	// The oldest chapter number still among the newest `count`.
	let cutoff = numbers.get(count - 1).or(numbers.last()).copied();

	chapters
		.iter()
		.map(|chapter| {
			let priced = chapter
				.price
				.as_deref()
				.and_then(|price| price.parse::<f32>().ok())
				.is_some_and(|price| price > 0.0);
			let public = match chapter.public_release.filter(|time| *time > 0) {
				Some(time) => now >= time,
				None if rolling => !chapter
					.number()
					.zip(cutoff)
					.is_some_and(|(number, cutoff)| number >= cutoff),
				None => true,
			};
			priced && !public
		})
		.collect()
}
