use crate::{
	api::{BASE_URL, CDN_URL},
	models::{ChapterItem, Scanlator, SearchDocument},
	settings::Keep,
};
use aidoku::{
	ContentRating, MangaStatus, Viewer,
	alloc::{String, Vec, format},
	imports::std::current_date,
};
use core::cmp::Ordering;

// Rating fallbacks for unrated titles.
pub const HENTAI: &str = "10";
pub const ADULT_GENRES: [&str; 2] = ["46", "41"];
pub const NOVEL_PREFIX: &str = "novel/";

const SPOTLIGHT_GENRES: [&str; 15] = [
	"Action",
	"Romance",
	"Fantasy",
	"Psychological",
	"Comedy",
	"Martial Arts",
	"Slice of Life",
	"Adventure",
	"Drama",
	"Sci-Fi",
	"Mystery",
	"Historical",
	"Supernatural",
	"Thriller",
	"Horror",
];

pub fn spotlight_genre() -> &'static str {
	SPOTLIGHT_GENRES[(current_date() / 86_400).rem_euclid(15) as usize]
}

pub fn content_rating<S: AsRef<str>>(
	is_adult: bool,
	rating: Option<&str>,
	genres: &[S],
) -> ContentRating {
	let tagged = |ids: &[&str]| genres.iter().any(|genre| ids.contains(&genre.as_ref()));
	if is_adult
		|| tagged(&[HENTAI])
		|| tagged(&ADULT_GENRES)
		|| matches!(rating, Some("Erotica" | "Pornographic"))
	{
		ContentRating::NSFW
	} else {
		match rating {
			Some("Suggestive") => ContentRating::Suggestive,
			Some("Safe") => ContentRating::Safe,
			_ => ContentRating::Unknown,
		}
	}
}

fn static_path(path: &str) -> &str {
	let path = path.trim_start_matches('/');
	path.strip_prefix("static/").unwrap_or(path)
}

// Every cover size has a webp copy beside the avif one.
pub fn cover_url(path: &str) -> String {
	let path = static_path(path);
	match path.strip_suffix(".avif") {
		Some(stem) => format!("{CDN_URL}/{stem}.webp"),
		None => format!("{CDN_URL}/{path}"),
	}
}

pub fn page_url(path: &str) -> String {
	format!("{CDN_URL}/{}", static_path(path))
}

// Lookup order differs from feed order.
pub fn take_document(documents: &mut Vec<SearchDocument>, id: &str) -> Option<SearchDocument> {
	let index = documents.iter().position(|document| document.id == id)?;
	Some(documents.swap_remove(index))
}

pub fn allows(values: &[String], value: &str) -> bool {
	values.iter().any(|allowed| allowed == value)
}

pub fn is_novel(medium: Option<&str>) -> bool {
	medium == Some("Novel")
}

pub fn manga_url(id: &str, novel: bool) -> String {
	let kind = if novel { "novel" } else { "manga" };
	format!("{BASE_URL}/{kind}/{id}")
}

pub fn chapter_url(manga_id: &str, chapter_id: &str, novel: bool) -> String {
	let kind = if novel { "novel" } else { "read" };
	format!("{BASE_URL}/{kind}/{manga_id}/{chapter_id}")
}

pub fn manga_key(id: String, novel: bool) -> String {
	if novel {
		format!("{NOVEL_PREFIX}{id}")
	} else {
		id
	}
}

pub fn manga_id(key: &str) -> &str {
	key.strip_prefix(NOVEL_PREFIX).unwrap_or(key)
}

pub fn parse_link(url: &str) -> Option<(String, Option<String>)> {
	let rest = url
		.strip_prefix("https://")
		.or_else(|| url.strip_prefix("http://"))?;
	let (host, path) = rest.split_once('/')?;
	if host != "atsu.moe" && host != "www.atsu.moe" {
		return None;
	}
	let path = path.split(['?', '#']).next()?;
	let mut parts = path.split('/').filter(|part| !part.is_empty());
	match (parts.next()?, parts.next()?, parts.next()) {
		("manga", id, None) => Some((id.into(), None)),
		(kind @ "novel", id, chapter) | (kind @ "read", id, chapter @ Some(_)) => Some((
			manga_key(id.into(), kind == "novel"),
			chapter.map(Into::into),
		)),
		_ => None,
	}
}

pub fn display_title(title: String, medium: Option<&str>, label_novels: bool) -> String {
	if label_novels && is_novel(medium) {
		format!("{title} (Novel)")
	} else {
		title
	}
}

pub fn viewer(kind: Option<&str>, medium: Option<&str>) -> Viewer {
	if is_novel(medium) {
		return Viewer::Unknown;
	}
	match kind {
		Some("Manga") => Viewer::RightToLeft,
		Some("Manwha" | "Manhua") => Viewer::Webtoon,
		_ => Viewer::Unknown,
	}
}

pub fn manga_status(status: Option<&str>) -> MangaStatus {
	match status.map(str::to_ascii_lowercase).as_deref() {
		Some("ongoing") => MangaStatus::Ongoing,
		Some("completed") => MangaStatus::Completed,
		Some("hiatus") => MangaStatus::Hiatus,
		Some("canceled" | "cancelled") => MangaStatus::Cancelled,
		_ => MangaStatus::Unknown,
	}
}

pub fn chapter_title(title: &str, number: Option<f32>) -> Option<String> {
	let trimmed = title.trim();
	if number.is_some() {
		let lower = trimmed.to_ascii_lowercase();
		for prefix in ["chapter", "episode", "ch.", "ep."] {
			if !lower.starts_with(prefix) {
				continue;
			}
			let rest = trimmed[prefix.len()..].trim_start();
			let after = rest.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.');
			if after.len() == rest.len() {
				break;
			}
			let name = after.trim_start_matches([' ', '-', ':', '–', '—']).trim();
			return Some(name.into()).filter(|name: &String| !name.is_empty());
		}
	}
	Some(trimmed.into()).filter(|title: &String| !title.is_empty())
}

pub fn arrange_chapters(
	mut chapters: Vec<ChapterItem>,
	scanlators: &[Scanlator],
	excluded: &[String],
	preferred: &[String],
	keep: Option<Keep>,
) -> Vec<ChapterItem> {
	if !excluded.is_empty() {
		chapters.retain(|chapter| {
			chapter
				.scanlator(scanlators)
				.is_none_or(|group| !excluded.contains(&group.name.to_lowercase()))
		});
	}
	// The site lists chapters newest first, so only deduplicating needs a sort.
	let Some(keep) = keep else {
		return chapters;
	};
	// Uploads without a scored group rank last.
	let score = |chapter: &ChapterItem| {
		chapter
			.scanlator(scanlators)
			.map_or(i32::MIN, |group| group.score)
	};
	let priority = |chapter: &ChapterItem| {
		chapter
			.scanlator(scanlators)
			.and_then(|group| {
				preferred.iter().position(|name| {
					name.chars()
						.eq(group.name.chars().flat_map(char::to_lowercase))
				})
			})
			.unwrap_or(usize::MAX)
	};
	chapters.sort_by(|a, b| {
		let by_number = match (a.number, b.number) {
			(Some(a), Some(b)) => b.partial_cmp(&a).unwrap_or(Ordering::Equal),
			(Some(_), None) => Ordering::Less,
			(None, Some(_)) => Ordering::Greater,
			(None, None) => Ordering::Equal,
		};
		let by_keep = match keep {
			Keep::HighestRated => score(b).cmp(&score(a)),
			Keep::MostPages => b.page_count.cmp(&a.page_count),
			Keep::Newest => Ordering::Equal,
		};
		by_number
			.then_with(|| priority(a).cmp(&priority(b)))
			.then(by_keep)
			.then(b.created_at.cmp(&a.created_at))
			.then_with(|| a.id.cmp(&b.id))
	});
	let mut selected = None;
	let mut group = None;
	chapters.retain(|chapter| {
		let Some(number) = chapter.number else {
			return true;
		};
		if selected == Some(number) {
			chapter.scanlation_manga_id == group
		} else {
			selected = Some(number);
			group = chapter.scanlation_manga_id.clone();
			true
		}
	});
	chapters
}

pub fn feed_allows(kind: Option<&str>, medium: Option<&str>, types: &[String]) -> bool {
	if is_novel(medium) {
		allows(types, "Novel")
	} else {
		kind.is_some_and(|kind| allows(types, kind))
	}
}
