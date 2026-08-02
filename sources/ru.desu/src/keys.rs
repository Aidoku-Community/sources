use aidoku::alloc::{String, string::ToString};
use aidoku::{Result, prelude::*};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Section {
	Manga,
	Ranobe,
}

pub fn manga_key(id: &str) -> String {
	if id.starts_with("m:") || id.starts_with("r:") {
		id.into()
	} else {
		format!("m:{id}")
	}
}

pub fn ranobe_key(slug: &str) -> String {
	if slug.starts_with("r:") {
		slug.into()
	} else {
		format!("r:{slug}")
	}
}

pub fn parse_key(key: &str) -> Result<(Section, String)> {
	if let Some(id) = key.strip_prefix("m:") {
		Ok((Section::Manga, id.into()))
	} else if let Some(slug) = key.strip_prefix("r:") {
		Ok((Section::Ranobe, slug.into()))
	} else if key.chars().all(|c| c.is_ascii_digit()) {
		// Legacy bare manga ids from v5.
		Ok((Section::Manga, key.into()))
	} else if key.contains('.') {
		Ok((Section::Ranobe, key.into()))
	} else {
		bail!("Неизвестный ключ: {key}")
	}
}

/// Extract `slug.id` from a ranobe path or href.
pub fn ranobe_slug(href: &str) -> Option<String> {
	let path = href
		.split('?')
		.next()
		.unwrap_or(href)
		.trim_start_matches('/')
		.trim_end_matches('/');
	let mut parts = path.split('/');
	if parts.next()? != "ranobe" {
		// relative without leading ranobe/ — first segment may be slug.id
		let first = path.split('/').next()?;
		return first.contains('.').then(|| first.to_string());
	}
	let slug = parts.next()?;
	slug.contains('.').then(|| slug.to_string())
}
