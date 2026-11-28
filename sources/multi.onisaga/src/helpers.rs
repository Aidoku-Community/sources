use crate::api::{BASE_URL, CDN_URL};
use aidoku::{
	ContentRating, MangaStatus, Viewer,
	alloc::{String, Vec, format},
	imports::std::{parse_date, parse_date_with_options},
};

pub const DEMOGRAPHICS: [&str; 5] = ["shounen", "shoujo", "seinen", "josei", "kodomo"];

#[derive(Debug, PartialEq)]
pub enum Link {
	Title(String),
	Chapter(String, String),
}

pub fn parse_link(url: &str) -> Option<Link> {
	let path = url.trim().strip_prefix(BASE_URL)?;
	let mut segs = path
		.split(['?', '#'])
		.next()?
		.split('/')
		.filter(|s| !s.is_empty());
	match (segs.next(), segs.next(), segs.next(), segs.next()) {
		(Some("title"), Some(slug), None, None) => Some(Link::Title(slug.into())),
		(Some("read"), Some(slug), Some(rel), None) if rel != "continue" => {
			Some(Link::Chapter(slug.into(), rel.into()))
		}
		_ => None,
	}
}

pub fn parse_status(value: &str) -> MangaStatus {
	match value.to_ascii_lowercase().as_str() {
		"releasing" => MangaStatus::Ongoing,
		"completed" => MangaStatus::Completed,
		"hiatus" => MangaStatus::Hiatus,
		"cancelled" => MangaStatus::Cancelled,
		_ => MangaStatus::Unknown,
	}
}

pub fn parse_content_rating(value: Option<&str>) -> ContentRating {
	match value.map(str::to_ascii_lowercase).as_deref() {
		Some("safe") => ContentRating::Safe,
		Some("suggestive") => ContentRating::Suggestive,
		Some("mature" | "explicit" | "erotica" | "pornographic") => ContentRating::NSFW,
		_ => ContentRating::Unknown,
	}
}

pub fn viewer(layout: Option<&str>, origin: Option<&str>, tags: &[String]) -> Viewer {
	match layout {
		Some("vertical_strip") => Viewer::Webtoon,
		Some("paged_rtl" | "yonkoma") => Viewer::RightToLeft,
		Some("paged_ltr") => Viewer::LeftToRight,
		_ if matches!(origin, Some("manhwa" | "manhua"))
			|| tags.iter().any(|tag| {
				tag.eq_ignore_ascii_case("Long Strip") || tag.eq_ignore_ascii_case("Web Comic")
			}) =>
		{
			Viewer::Webtoon
		}
		_ => match origin {
			Some("manga") => Viewer::RightToLeft,
			Some("comic" | "oel") => Viewer::LeftToRight,
			_ => Viewer::Unknown,
		},
	}
}

pub fn origin_name(slug: &str) -> Option<&'static str> {
	match slug {
		"manga" => Some("Manga"),
		"manhwa" => Some("Manhwa"),
		"manhua" => Some("Manhua"),
		"comic" => Some("Comic"),
		"oel" => Some("OEL"),
		"other_asian" => Some("Other Asian"),
		"other" => Some("Other"),
		_ => None,
	}
}

pub fn origin_slug(slug: &str) -> Option<&'static str> {
	match slug {
		"manga" => Some("manga"),
		"manhwa" => Some("manhwa"),
		"manhua" => Some("manhua"),
		"comic" => Some("comic"),
		"oel" => Some("oel"),
		"other-asian" | "other_asian" => Some("other_asian"),
		_ => None,
	}
}

// Named as the app names each status on a details page.
pub fn status_name(slug: &str) -> Option<&'static str> {
	match slug {
		"releasing" => Some("Ongoing"),
		"completed" => Some("Completed"),
		"hiatus" => Some("Hiatus"),
		"cancelled" => Some("Cancelled"),
		_ => None,
	}
}

pub fn demographic_name(slug: &str) -> Option<&'static str> {
	match slug {
		"shounen" => Some("Shounen"),
		"shoujo" => Some("Shoujo"),
		"seinen" => Some("Seinen"),
		"josei" => Some("Josei"),
		"kodomo" => Some("Kodomo"),
		_ => None,
	}
}

pub fn form_name(slug: &str) -> Option<&'static str> {
	match slug {
		"oneshot" => Some("One-shot"),
		"anthology" => Some("Anthology"),
		"doujinshi" => Some("Doujinshi"),
		"artbook" => Some("Art book"),
		_ => None,
	}
}

pub fn form_slug(slug: &str) -> Option<&'static str> {
	match slug {
		"serial" => Some("serial"),
		"oneshot" | "one-shot" => Some("oneshot"),
		"anthology" => Some("anthology"),
		"doujinshi" => Some("doujinshi"),
		"artbook" | "art-book" => Some("artbook"),
		_ => None,
	}
}

pub fn non_empty(value: String) -> Option<String> {
	let trimmed = value.trim();
	if trimmed.is_empty() {
		None
	} else if trimmed.len() == value.len() {
		Some(value)
	} else {
		Some(trimmed.into())
	}
}

pub fn dedup_keep_order(values: &mut Vec<String>) {
	let mut seen = Vec::with_capacity(values.len());
	values.retain(|value| {
		(!seen.contains(value))
			.then(|| seen.push(value.clone()))
			.is_some()
	});
}

fn strip_ascii_prefix<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
	let (head, tail) = text.split_at_checked(prefix.len())?;
	head.eq_ignore_ascii_case(prefix).then_some(tail)
}

fn strip_chapter_prefix(text: &str) -> Option<(&str, bool)> {
	let rest = text.split_once('·').map_or(text, |(_, after)| after).trim();
	if strip_ascii_prefix(rest, "vol").is_some() {
		return None;
	}
	for prefix in ["chapter", "chap", "ch", "episode", "ep", "side story"] {
		if let Some(tail) = strip_ascii_prefix(rest, prefix) {
			let tail = tail.strip_prefix('.').unwrap_or(tail);
			if tail.is_empty() || tail.starts_with(char::is_whitespace) {
				return Some((tail.trim(), true));
			}
		}
	}
	Some((rest, false))
}

pub fn parse_chapter_number(label: &str) -> Option<f32> {
	strip_chapter_prefix(label)?.0.parse().ok()
}

pub fn clean_chapter_title(
	title: Option<String>,
	label: &str,
	number: Option<f32>,
) -> Option<String> {
	if let Some(title) = title.and_then(non_empty)
		&& !title.eq_ignore_ascii_case(label.trim())
	{
		let mut rest = title.as_str();
		for vol in ["volume", "vol"] {
			if let Some(after) = strip_ascii_prefix(rest, vol) {
				let after = after.trim_start_matches('.').trim_start();
				let non_num = after.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.');
				if non_num.len() < after.len() {
					rest = non_num
						.trim_start_matches([' ', '-', ':', ',', '·'])
						.trim_start();
				}
				break;
			}
		}
		// Some uploads repeat the number, as in "Chapter 46 46".
		let duplicate = strip_chapter_prefix(rest).is_some_and(|(tail, prefixed)| {
			!tail.is_empty()
				&& tail.split_whitespace().all(|part| {
					part.parse::<f32>()
						.is_ok_and(|parsed| prefixed || Some(parsed) == number)
				})
		});
		if !duplicate {
			return Some(title);
		}
	}
	number
		.is_none()
		.then_some(label.trim())
		.and_then(|s| (!s.is_empty()).then(|| s.into()))
}

// OniSaga term slugs accept only `[a-z0-9]+(?:-[a-z0-9]+)*`.
pub fn slugify(name: &str) -> String {
	let mut ascii = String::with_capacity(name.len());
	for ch in name.chars() {
		match ch {
			_ if ch.is_ascii() => ascii.push(ch.to_ascii_lowercase()),
			'\'' | '\u{2019}' => {}
			'à'..='å' | 'ā' | 'À'..='Å' | 'Ā' => ascii.push('a'),
			'æ' | 'Æ' => ascii.push_str("ae"),
			'ç' | 'ć' | 'č' | 'Ç' | 'Ć' | 'Č' => ascii.push('c'),
			'è'..='ë' | 'ē' | 'È'..='Ë' | 'Ē' => ascii.push('e'),
			'ì'..='ï' | 'ī' | 'Ì'..='Ï' | 'Ī' => ascii.push('i'),
			'ñ' | 'ń' | 'Ñ' | 'Ń' => ascii.push('n'),
			'ò'..='ö' | 'ø' | 'ō' | 'Ò'..='Ö' | 'Ø' | 'Ō' => ascii.push('o'),
			'œ' | 'Œ' => ascii.push_str("oe"),
			'ß' => ascii.push_str("ss"),
			'ù'..='ü' | 'ū' | 'Ù'..='Ü' | 'Ū' => ascii.push('u'),
			'ý' | 'ÿ' | 'Ý' | 'Ÿ' => ascii.push('y'),
			_ => ascii.push(' '),
		}
	}
	ascii
		.split(|c: char| !c.is_ascii_alphanumeric())
		.filter(|word| !word.is_empty())
		.collect::<Vec<_>>()
		.join("-")
}

pub fn parse_terms(input: &str, include: &mut Vec<String>, exclude: &mut Vec<String>) {
	for raw in input.split(',') {
		let (target, name) = match raw.trim().strip_prefix(['-', '−']) {
			Some(name) => (&mut *exclude, name),
			None => (&mut *include, raw),
		};
		let slug = slugify(name);
		if !slug.is_empty() {
			target.push(slug);
		}
	}
}

fn clean_line(line: &str) -> String {
	let mut rest = line.trim().trim_start_matches(['#', '>']).trim();
	let mut out = String::with_capacity(rest.len());
	while let Some((before, after_open)) = rest.split_once('[') {
		if let Some((label, after_mid)) = after_open.split_once("](")
			&& let Some((_, after_close)) = after_mid.split_once(')')
		{
			out.push_str(before);
			out.push_str(label);
			rest = after_close;
			continue;
		}
		out.push_str(before);
		out.push('[');
		rest = after_open;
	}
	out.push_str(rest);
	out.retain(|c| c != '*' && c != '_' && c != '\\');
	out
}

pub fn plain_synopsis(text: &str) -> Option<String> {
	let mut joined = text
		.lines()
		.map(clean_line)
		.filter(|line| {
			let s = line.trim();
			!s.is_empty()
				&& !["(source:", "[source:", "source:"]
					.iter()
					.any(|p| strip_ascii_prefix(s, p).is_some())
		})
		.map(|line| String::from(line.trim()))
		.collect::<Vec<_>>()
		.join(" ");
	if let Some((before, tail)) = joined.rsplit_once('(')
		&& let Some(inner) = tail.strip_suffix(')')
		&& strip_ascii_prefix(inner.trim_start(), "source:").is_some()
	{
		joined.truncate(before.trim_end().len());
	}
	(!joined.is_empty()).then_some(joined)
}

// Safe pages carry `p=sg` and load from the CDN; gated pages need the main host's `__Host-` cookie,
// and with it confirmed the site hands them out as paths on that host.
pub fn media_url(url: String) -> String {
	if url.starts_with('/') {
		return format!("{BASE_URL}{url}");
	}
	let safe = url
		.split_once('?')
		.is_some_and(|(_, q)| q.split('&').any(|part| part == "p=sg"));
	if safe {
		url
	} else {
		url.replacen(CDN_URL, BASE_URL, 1)
	}
}

pub fn page_fragment(url: &str) -> Option<(&str, &str, i32)> {
	let (clean, fragment) = url.split_once("#oni=")?;
	let (release_id, ord) = fragment.split_once('.')?;
	Some((clean, release_id, ord.parse().ok()?))
}

pub fn page_expired(url: &str, now: i64) -> bool {
	url.split_once('?')
		.and_then(|(_, q)| q.split(['&', '#']).find_map(|p| p.strip_prefix("e=")))
		.and_then(|v| v.parse::<i64>().ok())
		.is_some_and(|expiry| now + 60 >= expiry)
}

pub fn parse_iso_date(time: &str) -> Option<i64> {
	parse_date_with_options(
		time,
		"yyyy-MM-dd'T'HH:mm:ss.SSSSSSXXX",
		"en_US_POSIX",
		"UTC",
	)
	.or_else(|| parse_date_with_options(time, "yyyy-MM-dd'T'HH:mm:ss.SSSXXX", "en_US_POSIX", "UTC"))
	.or_else(|| parse_date_with_options(time, "yyyy-MM-dd'T'HH:mm:ssXXX", "en_US_POSIX", "UTC"))
	.or_else(|| {
		// Fallback for hosts whose formatter drops fractional seconds and zones.
		let naive = time.split('.').next()?;
		parse_date(
			naive.trim_end_matches("+00:00").trim_end_matches('Z'),
			"yyyy-MM-ddTHH:mm:ss",
		)
	})
}
