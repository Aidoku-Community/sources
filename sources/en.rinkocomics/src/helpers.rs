use crate::models::AjaxResponse;
use aidoku::{
	Chapter, ContentRating, Manga, MangaPageResult, MangaStatus, Result,
	alloc::{String, Vec, format, string::ToString},
	helpers::uri::QueryParameters,
	imports::{
		html::{Document, Element, Html},
		net::{Request, Response},
		std::{current_date, parse_date_with_options},
	},
	prelude::*,
};
use base64::prelude::*;

pub const BASE_URL: &str = "https://rinkocomics.com";
pub const SORTS: [&str; 4] = ["az", "updated", "views", "new"];

pub fn get_html(url: &str) -> Result<Document> {
	checked_html(
		&Request::get(url)?
			.header("Referer", &format!("{BASE_URL}/"))
			.send()?,
	)
}

// An error page parses as an empty one, so a status other than 2xx is an error.
pub fn checked_html(response: &Response) -> Result<Document> {
	let status = response.status_code();
	if !(200..300).contains(&status) {
		bail!("Rinko Comics answered with HTTP {status}");
	}
	Ok(response.get_html()?)
}

pub fn browse_url(kind: &str, page: i32, qs: &QueryParameters) -> String {
	let mut url = format!("{BASE_URL}/{kind}/");
	if page > 1 {
		url.push_str(&format!("page/{page}/"));
	}
	if !qs.is_empty() {
		url.push_str(&format!("?{qs}"));
	}
	url
}

pub fn text(el: &Element, selector: &str) -> Option<String> {
	el.select_first(selector)?.text().filter(|s| !s.is_empty())
}

pub fn key_of(url: &str) -> Option<String> {
	let key = url
		.strip_prefix(BASE_URL)?
		.split(['?', '#'])
		.next()?
		.trim_matches('/');
	(!key.is_empty()).then(|| key.into())
}

pub fn slugify(name: &str) -> String {
	name.split(|c: char| !c.is_ascii_alphanumeric())
		.filter(|part| !part.is_empty())
		.map(str::to_ascii_lowercase)
		.collect::<Vec<_>>()
		.join("-")
}

pub fn cover(el: &Element) -> Option<String> {
	let img = el.select_first("img")?;
	img.attr("srcset")
		.and_then(|srcset| {
			srcset
				.split(',')
				.filter_map(|entry| {
					let (url, width) = entry.trim().split_once(' ')?;
					Some((width.trim_end_matches('w').parse::<u32>().ok()?, url.into()))
				})
				.max_by_key(|(width, _)| *width)
				.map(|(_, url)| url)
		})
		.or_else(|| img.attr("abs:src"))
}

pub fn card(link: &Element) -> Option<Manga> {
	let key = key_of(&link.attr("abs:href")?)?;
	Some(Manga {
		url: Some(format!("{BASE_URL}/{key}/")),
		title: text(link, ".line-clamp-2")?,
		key,
		cover: cover(link),
		content_rating: ContentRating::Safe,
		..Default::default()
	})
}

pub fn browse_page(doc: &Document) -> MangaPageResult {
	MangaPageResult {
		entries: doc
			.select("#cwBrowse ul > li > a")
			.map(|links| {
				links
					.filter_map(|link| {
						Some(Manga {
							status: text(&link, "span.text-subtle")
								.map_or(MangaStatus::Unknown, |s| parse_status(&s)),
							..card(&link)?
						})
					})
					.collect()
			})
			.unwrap_or_default(),
		has_next_page: doc.select_first("#cwBrowse a.next").is_some(),
	}
}

pub fn info(doc: &Document, label: &str) -> Option<String> {
	doc.select("main dl > div")?
		.find(|fact| text(fact, "dt").as_deref() == Some(label))
		.and_then(|fact| text(&fact, "dd"))
}

pub fn parse_status(status: &str) -> MangaStatus {
	match status.to_ascii_lowercase().as_str() {
		"ongoing" => MangaStatus::Ongoing,
		"completed" => MangaStatus::Completed,
		"season end" => MangaStatus::Hiatus,
		"dropped" => MangaStatus::Cancelled,
		_ => MangaStatus::Unknown,
	}
}

// Rows print "Sep 27, 2026", or an age ("7h", "1mo") on the home page.
pub fn parse_date(label: &str) -> Option<i64> {
	let label = label.split('·').next()?.trim();
	if let Some(idx) = label.find(|c: char| !c.is_ascii_digit())
		&& idx > 0
	{
		let unit = match &label[idx..] {
			"m" => 60,
			"h" => 3600,
			"d" => 86400,
			"w" => 604800,
			"mo" => 2592000,
			"y" => 31536000,
			_ => return None,
		};
		return Some(current_date() - label[..idx].parse::<i64>().ok()? * unit);
	}
	parse_date_with_options(label, "MMM d, yyyy", "en_US_POSIX", "current")
}

// One parser for every chapter row: series pages, their load-more fragments and the home list.
pub fn chapter(row: &Element) -> Option<Chapter> {
	// Paid rows link to "#" and keep their address in `data-permalink`.
	let locked = row.has_attr("data-reason");
	let url = row.attr(if locked {
		"abs:data-permalink"
	} else {
		"abs:href"
	})?;
	let spans = row.select("span")?.collect::<Vec<_>>();
	let label = spans
		.iter()
		.find_map(|span| Some(span.own_text()?.trim().into()).filter(|t: &String| !t.is_empty()))?;
	let number = label.trim_start_matches(|c: char| !c.is_ascii_digit());
	let end = number
		.find(|c: char| !c.is_ascii_digit() && c != '.')
		.unwrap_or(number.len());
	Some(Chapter {
		key: key_of(&url)?,
		title: text(row, "span.truncate").or_else(|| (end < number.len()).then(|| label.clone())),
		chapter_number: number[..end].trim_end_matches('.').parse().ok(),
		// From the end: a comic row's outer span also holds the label.
		date_uploaded: spans
			.iter()
			.rev()
			.find_map(|span| parse_date(&span.text()?)),
		url: Some(url),
		locked,
		..Default::default()
	})
}

pub fn script(doc: &Document, selector: &str) -> Option<String> {
	let el = doc.select_first(selector)?;
	el.data()
		.filter(|s| !s.is_empty())
		.or_else(|| el.html())
		.or_else(|| el.text())
}

pub fn chapter_list(doc: &Document, url: &str, show_locked: bool) -> Result<Vec<Chapter>> {
	let mut chapters = rows(doc, show_locked);
	// A long comic lists its first chapters and loads the rest a page at a time.
	if let Some(more) = doc.select_first("#cwMoreChapters") {
		let (Some(comic_id), Some(page_size), Some(total), Some(nonce)) = (
			more.attr("data-comic-id"),
			more.attr("data-offset")
				.and_then(|s| s.parse::<usize>().ok()),
			info(doc, "Chapters").and_then(|s| s.parse::<usize>().ok()),
			script(doc, "#comicworld-loadmore-js-extra").and_then(|s| {
				Some(String::from(
					s.split("\"nonce\":\"").nth(1)?.split('"').next()?,
				))
			}),
		) else {
			bail!("Rinko Comics changed its chapter list");
		};
		let requests = (page_size..total)
			.step_by(page_size.max(1))
			.map(|offset| {
				let mut form = QueryParameters::new();
				form.push_encoded("action", Some("load_more_chapters"));
				form.push("nonce", Some(&nonce));
				form.push("comic_id", Some(&comic_id));
				form.push_encoded("offset", Some(&offset.to_string()));
				form.push_encoded("order", Some("desc"));
				// Without it the site answers with an older row layout.
				form.push_encoded("ui", Some("v2"));
				Ok(
					Request::post(format!("{BASE_URL}/wp-admin/admin-ajax.php"))?
						.header("Content-Type", "application/x-www-form-urlencoded")
						.body(form.to_string()),
				)
			})
			.collect::<Result<Vec<_>>>()?;
		for response in Request::send_all(requests) {
			let html = response?.get_json_owned::<AjaxResponse>()?.data.html;
			chapters.extend(rows(
				&Html::parse_fragment_with_url(html, url)?,
				show_locked,
			));
		}
	}
	Ok(chapters)
}

fn rows(doc: &Document, show_locked: bool) -> Vec<Chapter> {
	// Comics list `a.chapter-item` rows; a novel lists every episode under `[data-ep]`.
	doc.select("a.chapter-item, [data-ep] > a")
		.map(|rows| {
			rows.filter_map(|row| chapter(&row))
				.filter(|chapter| show_locked || !chapter.locked)
				.collect()
		})
		.unwrap_or_default()
}

// The page list is base64: a 16-byte key, then a JSON array of urls XORed with it.
pub fn decode_pages(encoded: &str) -> Result<Vec<String>> {
	let bytes = BASE64_STANDARD
		.decode(encoded.trim())
		.map_err(|_| error!("Rinko Comics sent an unreadable page list"))?;
	let Some((key, data)) = bytes.split_at_checked(16) else {
		bail!("Rinko Comics sent an empty page list");
	};
	let json = data
		.iter()
		.enumerate()
		.map(|(i, b)| b ^ key[i % 16])
		.collect::<Vec<_>>();
	let pages: Vec<String> = serde_json::from_slice(&json)?;
	if pages.is_empty() || pages.iter().any(|page| page.is_empty()) {
		bail!("Rinko Comics sent an empty page list");
	}
	Ok(pages)
}
