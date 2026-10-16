use crate::{api::BASE_URL, models::Series};
use aidoku::{
	alloc::{String, format},
	imports::html::Html,
};

/// Demographics have their own filter, and the site never offers the rest for browsing.
const UNLISTED_TAGS: &[&str] = &[
	"Shounen", "Shoujo", "Seinen", "Josei", "Lolicon", "Shotacon", "Other",
];

pub fn is_unlisted_tag(tag: &str) -> bool {
	UNLISTED_TAGS.contains(&tag)
}

/// The site's own ladder: an adult flag or Hentai is pornographic, Smut and Adult are
/// erotica, Ecchi and Mature are suggestive.
pub fn rating_id(series: &Series) -> &'static str {
	let tags = series.tags.as_deref().unwrap_or_default();
	let tagged = |name: &str| tags.iter().any(|tag| tag == name);
	if series.adult == Some(true) || tagged("Hentai") {
		"pornographic"
	} else if tagged("Smut") || tagged("Adult") {
		"erotica"
	} else if tagged("Ecchi") || tagged("Mature") {
		"suggestive"
	} else {
		"safe"
	}
}

pub fn allows(series: &Series, ratings: &[String]) -> bool {
	let rating = rating_id(series);
	ratings.iter().any(|value| value == rating)
}

pub fn absolute_url(path: &str) -> String {
	if path.starts_with("http") {
		path.into()
	} else {
		format!("{BASE_URL}{path}")
	}
}

/// The catalogue serves the 460px cover; the 720px one sits beside it.
pub fn cover_url(cover: Option<&str>, large: bool) -> Option<String> {
	let cover = absolute_url(cover.filter(|cover| !cover.is_empty())?);
	Some(if large {
		cover.replace("/460.webp", "/720.webp")
	} else {
		cover
	})
}

pub fn manga_url(key: &str, slug: Option<&str>) -> String {
	match slug.filter(|slug| !slug.is_empty()) {
		Some(slug) => format!("{BASE_URL}/title/{key}-{slug}"),
		None => format!("{BASE_URL}/title/{key}"),
	}
}

/// The catalogue passes HTML entities through in its text.
pub fn clean_text(text: &str) -> String {
	let text = text.trim();
	if !text.contains('&') {
		return text.into();
	}
	Html::unescape(text).unwrap_or_else(|| text.into())
}

/// A series url is `/title/{key}-{slug}`, where a key is a lowercase letter then four
/// lowercase letters or digits, and a chapter's adds `/{id}-chapter-{number}`.
pub fn keys_from_url(url: &str) -> Option<(String, Option<String>)> {
	let mut segments = url.split("/title/").nth(1)?.split(['/', '?', '#']);
	let key = segments.next()?.split('-').next()?;
	let is_key = key.len() == 5
		&& key.starts_with(|c: char| c.is_ascii_lowercase())
		&& key
			.chars()
			.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
	if !is_key {
		return None;
	}
	let chapter = segments
		.next()
		.and_then(|segment| segment.split('-').next())
		.filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()));
	Some((key.into(), chapter.map(Into::into)))
}

/// The number in "Chapter 12", "Ch. 12", "#12" or "12", when that is all there is.
fn numbering(value: &str) -> Option<f64> {
	value
		.trim()
		.trim_start_matches(|c: char| c.is_ascii_alphabetic() || c == '.' || c == '#')
		.trim()
		.parse()
		.ok()
}

/// A label reads "Chapter 12: Title [Group]", where neither the numbering nor the
/// bracketed group is part of the title, and a title that only repeats the number is none.
pub fn split_label(label: &str, number: Option<f64>) -> (Option<String>, Option<String>) {
	let label = clean_text(label);
	let (label, group) = match (label.strip_suffix(']'), label.rfind('[')) {
		(Some(_), Some(open)) => (
			label[..open].trim(),
			Some(label[open + 1..label.len() - 1].trim()),
		),
		_ => (label.as_str(), None),
	};
	let title = match label.split_once([':', '-', '–', '—', '|']) {
		Some((leading, rest)) if numbering(leading).is_some() => rest.trim(),
		_ if numbering(label).is_some() => "",
		_ => label,
	};
	(
		(!title.is_empty() && (number.is_none() || numbering(title) != number))
			.then(|| title.into()),
		group.filter(|group| !group.is_empty()).map(Into::into),
	)
}
