use crate::{
	API_URL, BASE_URL, PAGE_SIZE,
	models::{ErrorResponse, Genre, GenreList, MirrorChapterList, MirrorList, NovelList},
	settings,
};
use aidoku::{
	Chapter, ContentRating, MangaPageResult, Result,
	alloc::{String, Vec, string::ToString},
	helpers::uri::QueryParameters,
	imports::net::{Request, Response},
	prelude::*,
};
use core::{fmt::Display, mem::take};
use serde::de::DeserializeOwned;

// The genres the site's library excludes when hiding adult novels.
pub const ADULT_GENRES: [&str; 12] = [
	"adult",
	"smut",
	"mature",
	"erotica",
	"ecchi",
	"hentai",
	"explicit",
	"sexual content",
	"nsfw",
	"r-18",
	"lewd",
	"pornographic",
];

// Values the genre list carries that the site treats as categories, not genres.
const NON_GENRES: [&str; 7] = [
	"browse",
	"completed novel",
	"completed novels",
	"latest novels",
	"latest novel",
	"anime & comics",
	"anime and comics",
];

pub fn request<T: AsRef<str>>(url: T) -> Result<Request> {
	Ok(Request::get(url)?.header("Accept", "application/json"))
}

pub fn get<T: DeserializeOwned>(url: &str) -> Result<T> {
	parse(request(url)?.send()?)
}

// The genres the filters and settings offer, without the site's category values.
pub fn listed_genres(list: GenreList) -> impl Iterator<Item = Genre> {
	list.genres
		.into_iter()
		.filter(|genre| !NON_GENRES.contains(&genre.value.to_lowercase().as_str()))
}

pub fn parse<T: DeserializeOwned>(response: Response) -> Result<T> {
	let status = response.status_code();
	if !(200..300).contains(&status) {
		match response.get_json_owned::<ErrorResponse>() {
			Ok(body) => bail!("Novel Archive: {}", body.error),
			Err(_) => bail!("Novel Archive returned status {status}"),
		}
	}
	response.get_json_owned()
}

#[derive(Default)]
pub struct SearchParams {
	pub query: Option<String>,
	pub sort: Option<&'static str>,
	pub status: Option<String>,
	pub source: Option<String>,
	pub any_genre: bool,
	pub genres: Vec<String>,
	pub hidden_genres: Vec<String>,
	pub hide_adult: bool,
	pub default_hidden_genres: Vec<String>,
}

impl SearchParams {
	pub fn from_settings() -> Self {
		Self {
			hide_adult: settings::hide_adult(),
			default_hidden_genres: settings::hidden_genres(),
			..Default::default()
		}
	}

	pub fn url(mut self, page: i32) -> String {
		// A genre the search itself includes or excludes wins over the setting.
		let default_hidden: Vec<String> = take(&mut self.default_hidden_genres)
			.into_iter()
			.filter(|genre| !self.genres.contains(genre) && !self.hidden_genres.contains(genre))
			.collect();
		self.hidden_genres.extend(default_hidden);
		if self.hide_adult {
			self.hidden_genres
				.extend(ADULT_GENRES.iter().map(|genre| genre.to_string()));
		}
		let mut qs = QueryParameters::new();
		qs.push("page", Some(&page.to_string()));
		qs.push("per_page", Some(&PAGE_SIZE.to_string()));
		if let Some(query) = &self.query {
			qs.push("search", Some(query));
			qs.push("fuzzy", Some("1"));
		}
		if let Some(sort) = self.sort {
			qs.push("sort", Some(sort));
		}
		if let Some(status) = &self.status {
			qs.push("status", Some(status));
		}
		if let Some(source) = &self.source {
			qs.push("source", Some(source));
		}
		if !self.genres.is_empty() {
			qs.push("genres_include", Some(&self.genres.join(",")));
		}
		if !self.hidden_genres.is_empty() {
			qs.push("genres_exclude", Some(&self.hidden_genres.join(",")));
		}
		if self.any_genre {
			qs.push("genre_match", Some("any"));
		}
		format!("{API_URL}?{qs}")
	}
}

pub fn browse(params: SearchParams, page: i32) -> Result<MangaPageResult> {
	let list: NovelList = get(&params.url(page))?;
	Ok(MangaPageResult {
		has_next_page: list
			.pagination
			.is_some_and(|pagination| pagination.has_next),
		entries: list.novels.into_iter().map(Into::into).collect(),
	})
}

pub fn chapter_url(id: &str, number: impl Display) -> String {
	format!("{BASE_URL}/reader?novel={id}&chapter={number}")
}

pub fn genres(list: &str) -> Vec<String> {
	let mut genres: Vec<String> = Vec::new();
	for genre in list.split(',').map(str::trim) {
		let lowered = genre.to_lowercase();
		if genre.is_empty()
			|| NON_GENRES.contains(&lowered.as_str())
			|| genres.iter().any(|seen| seen.to_lowercase() == lowered)
		{
			continue;
		}
		genres.push(genre.into());
	}
	genres
}

pub fn content_rating(list: &str) -> ContentRating {
	let mut rating = ContentRating::Safe;
	for genre in list.split(',').map(|genre| genre.trim().to_lowercase()) {
		match genre.as_str() {
			"mature" | "ecchi" => rating = ContentRating::Suggestive,
			genre if ADULT_GENRES.contains(&genre) => return ContentRating::NSFW,
			_ => {}
		}
	}
	rating
}

// "Chapter 2315 Nothing Is Certain" gives 2315 and "Nothing Is Certain"; a name without a leading
// number keeps the fallback, and a title that is only a part number ("2") is none.
pub fn number_and_title(name: &str, fallback: f32) -> (f32, Option<String>) {
	let name = name.trim();
	let (number, title) = leading_chapter_number(name).unwrap_or((fallback, name));
	let title = (!title.chars().all(|c| c.is_ascii_digit())).then(|| title.into());
	(number, title)
}

fn leading_chapter_number(name: &str) -> Option<(f32, &str)> {
	let (start, rest) = name.split_at_checked("chapter ".len())?;
	if !start.eq_ignore_ascii_case("chapter ") {
		return None;
	}
	let (digits, title) = rest.split_at(
		rest.find(|c: char| !c.is_ascii_digit() && c != '.')
			.unwrap_or(rest.len()),
	);
	let number = digits.trim_end_matches('.').parse().ok()?;
	let separators = [' ', ':', '-', '\u{2013}', '\u{2014}', '.'];
	let title = title.trim_start_matches(separators);
	// Some names repeat the number: "Chapter 2 - 2: The Joy of Being".
	let title = title
		.strip_prefix(digits)
		.filter(|rest| rest.starts_with(separators))
		.map_or(title, |rest| rest.trim_start_matches(separators));
	Some((number, title.trim_end()))
}

// Mirrors are extra: when their list is slow or fails, the site's own chapters still load.
pub fn mirror_chapters(id: &str) -> Vec<Chapter> {
	let chapters = || -> Result<Vec<Chapter>> {
		// The list can hang for half a minute before a 504.
		let list: MirrorList = parse(
			request(format!("{API_URL}/{id}/sources"))?
				.timeout(8.0)
				.send()?,
		)?;
		let mut requests = Vec::new();
		for mirror in &list.sources {
			requests.push(request(format!(
				"{API_URL}/{id}/sources/{}/chapters",
				mirror.id
			))?);
		}
		Ok(list
			.sources
			.iter()
			.zip(Request::send_all(requests))
			.filter_map(|(mirror, response)| {
				Some((mirror, parse::<MirrorChapterList>(response.ok()?).ok()?))
			})
			.flat_map(|(mirror, chapters)| {
				chapters
					.chapters
					.into_iter()
					.map(|chapter| chapter.into_chapter(id, mirror))
			})
			.collect())
	};
	chapters().unwrap_or_default()
}
