use crate::settings;
use aidoku::{
	Chapter, ContentRating, Manga, MangaPageResult, MangaStatus, Result, Viewer,
	alloc::{String, Vec, format, string::ToString},
	helpers::uri::QueryParameters,
	imports::{
		html::{Document, Element},
		net::{Request, Response},
		std::{current_date, parse_date_with_options},
	},
	prelude::*,
};

pub const SORTS: [&str; 4] = ["viewed", "scored", "created_at", "updated_at"];

pub fn manga_url(key: &str) -> String {
	format!("{}/manga/{key}", settings::base_url())
}

pub fn manga_key(url: &str) -> Option<String> {
	url.split_once("/manga/")?
		.1
		.split(['/', '?', '#'])
		.next()
		.filter(|key| !key.is_empty())
		.map(Into::into)
}

/// Finishes a search with its page and genres. The genres excluded in settings apply
/// unless the search includes them.
pub fn search_url(
	mut qs: QueryParameters,
	page: i32,
	included: &[String],
	excluded: &[String],
) -> String {
	qs.push_encoded("page", Some(&page.to_string()));
	for genre in included {
		qs.push("genre[]", Some(genre));
	}
	for genre in settings::excluded_genres().iter().chain(excluded) {
		if !included.contains(genre) {
			qs.push("exclude_genre[]", Some(genre));
		}
	}
	format!("{}/search?{qs}", settings::base_url())
}

pub fn sorted_url(sort: &str, page: i32) -> String {
	let mut qs = QueryParameters::new();
	qs.push_encoded("sort", Some(sort));
	search_url(qs, page, &[], &[])
}

pub fn parse_html(response: &Response) -> Result<Document> {
	let status = response.status_code();
	if status != 200 {
		bail!("VyManga answered with HTTP {status}");
	}
	Ok(response.get_html()?)
}

pub fn get_html(url: &str) -> Result<Document> {
	parse_html(&Request::get(url)?.send()?)
}

pub fn parse_page(html: &Document) -> MangaPageResult {
	MangaPageResult {
		entries: html
			.select(".comic-item")
			.map(|cards| {
				cards
					.filter_map(|card| {
						Some(Manga {
							key: manga_key(&card.select_first("a")?.attr("href")?)?,
							title: card
								.select_first(".comic-title")?
								.text()
								.filter(|title| !title.is_empty())?,
							cover: card
								.select_first("img")
								.and_then(|img| img.attr("data-src")),
							content_rating: if card.select_first(".cover-adult").is_some() {
								ContentRating::NSFW
							} else {
								ContentRating::Unknown
							},
							..Default::default()
						})
					})
					.collect()
			})
			.unwrap_or_default(),
		has_next_page: html.select_first("[rel=next]").is_some(),
	}
}

// A series page links each name; a home slide prints them as text joined by " , ".
fn names(row: &Element) -> Option<Vec<String>> {
	let listed = row
		.select("a")
		.map(|links| links.filter_map(|link| link.text()).collect::<Vec<_>>())
		.filter(|links| !links.is_empty())
		.or_else(|| {
			let text = row.text()?;
			Some(
				text.split_once(':')?
					.1
					.split(" ,")
					.map(Into::into)
					.collect(),
			)
		})?;
	let mut names: Vec<String> = Vec::new();
	for name in listed {
		let name = name.trim_matches([' ', ',']);
		if !name.is_empty() && name != "--" && !names.iter().any(|listed| listed == name) {
			names.push(name.into());
		}
	}
	(!names.is_empty()).then_some(names)
}

/// Reads a series from its page or from a home slide, which share their markup.
pub fn parse_manga(key: String, info: &Element) -> Result<Manga> {
	let mut manga = Manga {
		url: Some(manga_url(&key)),
		key,
		title: info
			.select_first("h1.title")
			.and_then(|title| title.text())
			.filter(|title| !title.is_empty())
			.ok_or_else(|| error!("Missing title"))?,
		cover: info
			.select_first(".img-manga img")
			.and_then(|cover| cover.attr("src")),
		description: info
			.select_first(".desc, .summary > .content")
			.and_then(|description| description.text())
			.filter(|description| !description.is_empty()),
		..Default::default()
	};
	for label in info.select(".pre-title").into_iter().flatten() {
		let Some(row) = label.parent() else {
			continue;
		};
		match label.text().as_deref() {
			Some("Authors") => manga.authors = names(&row),
			Some("Artists") => manga.artists = names(&row),
			Some("Status") => {
				let status = row.select_first("span:last-child").and_then(|el| el.text());
				manga.status = match status.map(|status| status.to_lowercase()).as_deref() {
					Some("ongoing") => MangaStatus::Ongoing,
					Some("completed") => MangaStatus::Completed,
					_ => MangaStatus::Unknown,
				};
			}
			Some("Genres") => {
				manga.tags = row
					.select(".label-badge")
					.map(|tags| tags.filter_map(|tag| tag.text()).collect::<Vec<_>>())
					.filter(|tags| !tags.is_empty());
			}
			_ => {}
		}
	}

	let tags = manga.tags.as_deref().unwrap_or_default();
	let tagged = |names: &[&str]| {
		tags.iter()
			.any(|tag| names.iter().any(|name| tag.eq_ignore_ascii_case(name)))
	};
	manga.content_rating = if info.select_first(".img-manga .cover-adult").is_some()
		|| tagged(&["Erotica", "Pornographic", "R-18", "Smut"])
	{
		ContentRating::NSFW
	} else if tagged(&["Ecchi", "Mature"]) {
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	};
	manga.viewer = if tagged(&["Long strip", "Manhua", "Manhwa", "Webtoons"]) {
		Viewer::Webtoon
	} else if tagged(&["Manga"]) {
		Viewer::RightToLeft
	} else {
		Viewer::Unknown
	};
	Ok(manga)
}

fn take_number(text: &mut &str, label: &str) -> Option<f32> {
	let rest = text.strip_prefix(label)?;
	let end = rest
		.find(|c: char| !c.is_ascii_digit() && c != '.')
		.unwrap_or(rest.len());
	let number = rest[..end].parse().ok()?;
	*text = rest[end..].trim_start_matches([' ', ':']);
	Some(number)
}

fn parse_chapter_date(text: &str) -> Option<i64> {
	let Some(elapsed) = text.strip_suffix(" ago") else {
		return parse_date_with_options(text, "MMM dd, yyyy", "en_US_POSIX", "UTC");
	};
	let (amount, unit) = elapsed.split_once(' ')?;
	let seconds = match unit.trim_end_matches('s') {
		"second" => 1,
		"minute" => 60,
		"hour" => 3600,
		"day" => 86400,
		_ => return None,
	};
	Some(current_date() - amount.parse::<i64>().ok()? * seconds)
}

pub fn parse_chapters(html: &Document) -> Vec<Chapter> {
	let links: Vec<(String, Element)> = html
		.select(".list-chapter")
		.map(|links| {
			links
				.filter_map(|link| Some((link.select_first("span")?.text()?, link)))
				.collect()
		})
		.unwrap_or_default();
	links
		.iter()
		.enumerate()
		.map(|(index, (name, link))| {
			// The site gives chapters no id and encrypts each link anew on every load, so
			// the name is the key, counted from the oldest when a name repeats.
			let repeats = links[index + 1..]
				.iter()
				.filter(|(other, _)| other == name)
				.count();
			let mut title = name.as_str();
			let volume_number = take_number(&mut title, "Vol.");
			let chapter_number = take_number(&mut title, "Chapter ");
			let repeats_number = title
				.strip_prefix("Ch ")
				.and_then(|number| number.parse::<f32>().ok())
				.is_some_and(|number| Some(number) == chapter_number);
			Chapter {
				key: if repeats == 0 {
					name.clone()
				} else {
					format!("{name}#{}", repeats + 1)
				},
				title: (!title.is_empty() && !repeats_number).then(|| title.into()),
				chapter_number,
				volume_number,
				date_uploaded: link
					.select_first("p")
					.and_then(|date| date.text())
					.and_then(|date| parse_chapter_date(&date)),
				url: link.attr("href"),
				..Default::default()
			}
		})
		.collect()
}
