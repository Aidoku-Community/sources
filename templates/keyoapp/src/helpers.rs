use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, Result, Viewer,
	alloc::{String, Vec, string::ToString},
	helpers::string::StripPrefixOrSelf,
	imports::{
		html::{Document, Element, Kind},
		net::{Request, Response},
		std::{current_date, parse_date_with_options},
	},
	prelude::*,
};
use serde::de::DeserializeOwned;

pub fn fetch(url: &str) -> Result<Response> {
	let response = Request::get(url)?.send()?;
	let status = response.status_code();
	if !(200..300).contains(&status) {
		log_response(&response, url);
		bail!("HTTP {status}");
	}
	Ok(response)
}

pub fn fetch_json<T: DeserializeOwned>(url: &str) -> Result<T> {
	let mut response = fetch(url)?;
	let result = response.get_json();
	if result.is_err() {
		log_response(&response, url);
	}
	result
}

pub fn log_response(response: &Response, url: &str) {
	let status = response.status_code();
	let content_type = response.get_header("Content-Type");
	let prefix = response
		.get_string()
		.ok()
		.map(|body| body.chars().take(240).collect::<String>());
	println!("URL: {url}; HTTP {status}; Content-Type: {content_type:?}; body prefix: {prefix:?}");
}

// Latest posters include a blurred backdrop outside the cover link.
pub fn manga_from_link(link: &Element, card: &Element, base_url: &str) -> Option<Manga> {
	Some(Manga {
		key: series_key(link.attr("href")?.strip_prefix_or_self(base_url))?,
		title: link
			.attr("title")
			.filter(|title| !title.trim().is_empty())?
			.trim()
			.into(),
		cover: link
			.select_first("[style*=background-image]")
			.or_else(|| card.select_first("[style*=background-image]"))
			.and_then(|cover| background_image(&cover)),
		..Default::default()
	})
}

pub fn background_image(element: &Element) -> Option<String> {
	let style = element.attr("style")?;
	let url = style.split_once("url(")?.1.split_once(')')?.0;
	Some(url.trim_matches(['\'', '"', ' ']).into())
}

// The runner cannot walk siblings, so read labels through the block's children.
pub fn labelled(html: &Document, label: &str) -> Option<String> {
	html.select("div.grid")?.find_map(|block| {
		let mut children = block.children();
		let name = children.next()?.select_first("span")?.text()?;
		if name.trim() != label {
			return None;
		}
		children
			.next()?
			.text()
			.map(|text| text.trim().to_string())
			.filter(|text| !text.is_empty())
	})
}

pub fn names(text: &str) -> Vec<String> {
	text.split(", ").map(|name| name.trim().into()).collect()
}

pub fn series_key(path: &str) -> Option<String> {
	let slug = path
		.split(['?', '#'])
		.next()?
		.strip_prefix("/series/")?
		.trim_end_matches('/');
	(!slug.is_empty()).then(|| format!("/series/{slug}/"))
}

pub fn status(text: &str) -> MangaStatus {
	match text.to_lowercase().as_str() {
		"ongoing" => MangaStatus::Ongoing,
		"completed" => MangaStatus::Completed,
		"dropped" => MangaStatus::Cancelled,
		"hiatus" | "paused" => MangaStatus::Hiatus,
		_ => MangaStatus::Unknown,
	}
}

pub fn chapter_parts(label: &str) -> (Option<f32>, Option<String>) {
	let label = label.trim();
	let Some(rest) = label.strip_prefix("Chapter") else {
		return (None, Some(label.into()));
	};
	let rest = rest.trim_start();
	let end = rest
		.find(|c: char| !(c.is_ascii_digit() || c == '.'))
		.unwrap_or(rest.len());
	let name = rest[end..].trim_start_matches([' ', '-', ':']).trim();
	(
		rest[..end].parse().ok(),
		(!name.is_empty()).then(|| name.into()),
	)
}

// V2 abbreviates relative dates; older chapters use absolute dates.
pub fn chapter_date(text: &str) -> Option<i64> {
	let text = text.trim();
	let Some(ago) = text.strip_suffix("ago") else {
		return parse_date_with_options(text, "MMM d, yyyy", "en_US_POSIX", "current");
	};
	let ago = ago.trim();
	let digits = ago.find(|c: char| !c.is_ascii_digit()).unwrap_or(ago.len());
	let amount: i64 = ago[..digits].parse().ok()?;
	let unit = ago[digits..].trim();
	let seconds = if unit == "m" || unit.starts_with("min") {
		60
	} else if unit.starts_with('h') {
		60 * 60
	} else if unit.starts_with('d') {
		24 * 60 * 60
	} else {
		return None;
	};
	Some(current_date() - amount * seconds)
}

pub fn content_rating<'a>(tags: impl Iterator<Item = &'a str> + Clone) -> ContentRating {
	let has_any = |names: &[&str]| {
		tags.clone()
			.any(|tag| names.iter().any(|name| tag.eq_ignore_ascii_case(name)))
	};
	if has_any(&["Adult", "Smut", "Hentai"]) {
		ContentRating::NSFW
	} else if has_any(&["Ecchi", "Mature"]) {
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	}
}

pub fn viewer(kind: &str) -> Viewer {
	match kind.to_lowercase().as_str() {
		"manga" => Viewer::RightToLeft,
		"comic" => Viewer::LeftToRight,
		"manhwa" | "manhua" | "mangatoon" => Viewer::Webtoon,
		_ => Viewer::Unknown,
	}
}

// Separate paragraphs avoid the reader's inline-node stack limit.
pub fn novel_text(reader: &Element) -> Option<String> {
	let mut paragraphs = Vec::new();
	for paragraph in reader.select("p")? {
		let mut text = String::new();
		inline_markdown(&paragraph, &mut text);
		paragraphs.extend(
			text.split("\n\n")
				.map(str::trim)
				.filter(|part| !part.is_empty())
				.map(String::from),
		);
	}
	(!paragraphs.is_empty()).then(|| paragraphs.join("\n\n"))
}

// Text comes from child nodes and tags from children, whose elements the test runner can name
fn inline_markdown(element: &Element, output: &mut String) {
	let mut children = element.children();
	for node in element.child_nodes() {
		match node.kind() {
			Kind::TextNode => {
				if let Some(text) = node.text() {
					// Ordinary punctuation would add needless inline nodes in the reader.
					for c in text.chars() {
						let block = match c {
							'#' | '-' | '+' | '=' | '.' | ')' => {
								let line =
									output.rsplit('\n').next().unwrap_or_default().trim_start();
								if matches!(c, '.' | ')') {
									!line.is_empty()
										&& line.bytes().all(|byte| byte.is_ascii_digit())
								} else {
									line.is_empty()
								}
							}
							_ => false,
						};
						if block
							|| matches!(
								c,
								'\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '&' | '~' | '|'
							) {
							output.push('\\');
						}
						output.push(c);
					}
				}
			}
			Kind::Element => {
				let Some(child) = children.next() else {
					continue;
				};
				let marker = match child.tag_name().as_deref() {
					Some("br") => {
						output.push_str("\n\n");
						continue;
					}
					Some("strong" | "b") => "**",
					Some("em" | "i") => "*",
					_ => "",
				};
				let mut inner = String::new();
				inline_markdown(&child, &mut inner);
				if marker.is_empty() {
					output.push_str(&inner);
					continue;
				}
				for (index, part) in inner.split("\n\n").enumerate() {
					if index > 0 {
						output.push_str("\n\n");
					}
					let trimmed = part.trim();
					if trimmed.is_empty() {
						output.push_str(part);
						continue;
					}
					// Whitespace outside the markers keeps Markdown emphasis valid.
					let start = part.len() - part.trim_start().len();
					output.push_str(&part[..start]);
					output.push_str(marker);
					output.push_str(trimmed);
					output.push_str(marker);
					output.push_str(&part[start + trimmed.len()..]);
				}
			}
			_ => {}
		}
	}
}

pub fn chapter_from_link(link: &Element, base_url: &str) -> Option<Chapter> {
	// Coin chapters require login; upcoming chapters cannot be read yet.
	if link.select_first("img[alt=Coin]").is_some()
		|| link.select("span").is_some_and(|mut spans| {
			spans.any(|span| {
				span.own_text()
					.is_some_and(|text| text.trim() == "Upcoming")
			})
		}) {
		return None;
	}
	let key: String = link.attr("href")?.strip_prefix_or_self(base_url).into();
	let (chapter_number, title) = chapter_parts(&link.attr("title")?);
	Some(Chapter {
		url: Some(format!("{base_url}{key}")),
		key,
		title,
		chapter_number,
		date_uploaded: link.attr("d").and_then(|date| chapter_date(&date)),
		..Default::default()
	})
}

// The alternate series layout stores its cover only in metadata.
pub fn manga_from_page(html: &Document) -> Option<Manga> {
	html.select_first("[data-series-uid]")?;
	let slug = html
		.select_first("[data-series-slug]")?
		.attr("data-series-slug")?;
	Some(Manga {
		key: format!("/{slug}"),
		title: html
			.select_first("h1.l2-title")
			.or_else(|| html.select_first("h1"))?
			.text()?
			.trim()
			.into(),
		cover: html
			.select_first(".l2-poster img")
			.and_then(|image| image.attr("data-src"))
			.or_else(|| {
				html.select_first("meta[property='og:image']")?
					.attr("content")
			}),
		..Default::default()
	})
}

pub fn labelled_block(html: &Document, label: &str) -> Option<String> {
	html.select("div.bg-surface.rounded-lg")?.find_map(|block| {
		let name = block.select_first(".text-xs")?.text()?;
		if name.trim() != label {
			return None;
		}
		block
			.select_first("span.text-sm")?
			.text()
			.map(|value| value.trim().to_string())
			.filter(|value| !value.is_empty())
	})
}

pub fn manga_from_card(card: &Element, base_url: &str) -> Option<Manga> {
	Some(Manga {
		key: card.attr("href")?.strip_prefix_or_self(base_url).into(),
		title: card
			.attr("title")
			.filter(|title| !title.trim().is_empty())
			.or_else(|| card.select_first("h2, h3")?.text())
			.filter(|title| !title.trim().is_empty())?
			.trim()
			.into(),
		cover: card
			.select_first("img")
			.and_then(|image| image.attr("data-src")),
		..Default::default()
	})
}
