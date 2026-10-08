use crate::auth::{AuthedRequest, require_login};
use crate::helpers::{apply_headers, get_base_url};
use crate::keys::{ranobe_key, ranobe_slug};
use crate::models::{DesuChapter, DesuChaptersResponse};
use crate::settings::{eng_title, path_on_site, ranobe_cover_preview, rewrite_media_url};
use aidoku::{
	Chapter, ContentRating, FilterValue, Manga, MangaPageResult, MangaStatus, Page, PageContent,
	Result, Viewer,
	alloc::{String, Vec, string::ToString},
	helpers::uri::QueryParameters,
	imports::{
		html::{Document, Element, Html},
		net::Request,
	},
	prelude::*,
};
use alloc::vec;
use serde::Deserialize;

const RANOBE_PAGE_SIZE: i32 = 24;

#[derive(Deserialize)]
struct RanobeChapterResponse {
	chapter: Option<RanobeApiChapter>,
}

#[derive(Deserialize)]
struct RanobeApiChapter {
	content: Option<Vec<RanobeContentBlock>>,
}

#[derive(Deserialize)]
struct RanobeContentBlock {
	#[serde(rename = "type")]
	content_type: Option<String>,
	html: Option<String>,
	url: Option<String>,
}

fn ranobe_id_from_slug(slug: &str) -> Option<&str> {
	let (_, id) = slug.rsplit_once('.')?;
	(!id.is_empty() && id.chars().all(|c| c.is_ascii_digit())).then_some(id)
}

fn absolute_view_url(base: &str, url: &str) -> String {
	if url.starts_with("http://") || url.starts_with("https://") {
		rewrite_media_url(url)
	} else {
		format!("{base}/{}", url.trim_start_matches('/'))
	}
}

fn is_ranobe_block_tag(tag: &str) -> bool {
	matches!(
		tag,
		"article"
			| "blockquote"
			| "dd" | "div"
			| "dl" | "dt"
			| "h1" | "h2"
			| "h3" | "h4"
			| "h5" | "h6"
			| "hr" | "li"
			| "ol" | "p"
			| "pre" | "section"
			| "table" | "tbody"
			| "td" | "tfoot"
			| "th" | "thead"
			| "tr" | "ul"
	)
}

fn preserve_ranobe_text_formatting(html: &str) -> String {
	let bytes = html.as_bytes();
	let mut output = String::with_capacity(html.len());
	let mut index = 0;

	while index < bytes.len() {
		if bytes[index] == b'<'
			&& let Some(relative_end) = bytes[index..].iter().position(|byte| *byte == b'>')
		{
			let end = index + relative_end;
			let tag = html[index + 1..end]
				.trim_start_matches('/')
				.trim_start()
				.split(|character: char| character == '/' || character.is_whitespace())
				.next()
				.unwrap_or_default()
				.to_ascii_lowercase();
			match tag.as_str() {
				"br" => output.push('\n'),
				"b" | "strong" => output.push_str("**"),
				"i" | "em" => output.push('*'),
				_ if is_ranobe_block_tag(&tag) => output.push_str("\n\n"),
				_ => output.push_str(&html[index..=end]),
			}
			index = end + 1;
			continue;
		}

		let character = html[index..].chars().next().unwrap();
		output.push(character);
		index += character.len_utf8();
	}

	output
}

fn normalize_ranobe_text(text: &str) -> String {
	let mut output = String::with_capacity(text.len());
	let mut pending_newlines = 0_usize;

	for character in text.chars() {
		if character == '\r' {
			continue;
		}
		if character == '\n' {
			while output.ends_with(' ') || output.ends_with(char::from(9)) {
				output.pop();
			}
			pending_newlines = (pending_newlines + 1).min(2);
			continue;
		}
		if pending_newlines > 0 && character.is_whitespace() {
			continue;
		}
		if pending_newlines > 0 {
			output.push_str(if pending_newlines == 1 { "\n" } else { "\n\n" });
			pending_newlines = 0;
		}
		output.push(character);
	}

	output.trim().to_string()
}

fn text_html(block: &RanobeContentBlock) -> Option<&str> {
	match block.content_type.as_deref() {
		Some("image") => None,
		Some("text") | None => block.html.as_deref(),
		Some(_) => None,
	}
}

fn chapter_text_from_content(content: &[RanobeContentBlock]) -> Result<Option<String>> {
	let mut blocks = Vec::new();
	for block in content {
		let Some(html) = text_html(block) else {
			continue;
		};
		let html = preserve_ranobe_text_formatting(html);
		let wrapped_html = alloc::format!("<div id='desu-ranobe-reader-root'>{html}</div>");
		let document = Html::parse_fragment(&wrapped_html)
			.map_err(|_| error!("Не удалось разобрать текст главы Desu"))?;
		let text = document
			.select_first("#desu-ranobe-reader-root")
			.and_then(|root| root.untrimmed_text())
			.map(|text| normalize_ranobe_text(&text))
			.unwrap_or_default();
		if !text.is_empty() {
			blocks.push(text);
		}
	}

	let text = blocks.join("\n\n");
	Ok((!text.is_empty()).then_some(text))
}

fn chapter_pages_from_content(content: &[RanobeContentBlock], base: &str) -> Result<Vec<Page>> {
	if let Some(text) = chapter_text_from_content(content)? {
		return Ok(vec![Page {
			content: PageContent::text(text),
			..Default::default()
		}]);
	}

	let pages = content
		.iter()
		.filter_map(|block| match block.content_type.as_deref() {
			Some("image") | None => block.url.as_deref(),
			Some(_) => None,
		})
		.filter(|url| !url.is_empty())
		.map(|url| Page {
			content: PageContent::url(absolute_view_url(base, url)),
			..Default::default()
		})
		.collect::<Vec<_>>();
	if pages.is_empty() {
		bail!("Текст или изображения главы не найдены в API-ответе Desu");
	}
	Ok(pages)
}

fn ranobe_chapters_from_api(chapters: Vec<DesuChapter>) -> Vec<Chapter> {
	chapters
		.into_iter()
		.filter(|chapter| chapter.is_readable.unwrap_or(true))
		.map(Chapter::from)
		.collect()
}

fn normalized_view_path(base: &str, url: &str) -> Option<String> {
	let absolute = absolute_view_url(base, url);
	let path = path_on_site(&absolute)?;
	let path = path.split(['?', '#']).next().unwrap_or(&path);
	Some(path.trim_matches('/').to_string())
}

fn ranobe_chapter_deep_link_url(base: &str, path: &str, slug: &str) -> Option<String> {
	let path = path.split(['?', '#']).next()?.trim_start_matches('/');
	let prefix = format!("ranobe/{slug}/");
	let chapter_path = path.strip_prefix(&prefix)?;
	(!chapter_path.trim_matches('/').is_empty()).then(|| format!("{base}/{path}"))
}

fn chapter_id_from_api_view_url(
	chapters: &[DesuChapter],
	base: &str,
	target_url: &str,
) -> Option<String> {
	let target_path = normalized_view_path(base, target_url)?;
	chapters.iter().find_map(|chapter| {
		let candidate = chapter.view_url.as_deref()?;
		(normalized_view_path(base, candidate).as_deref() == Some(target_path.as_str()))
			.then(|| chapter.id.to_string())
	})
}

fn fetch_ranobe_chapter_records(book_id: &str) -> Result<Vec<DesuChapter>> {
	let url = format!("{}/api/ranobe/{book_id}/chapters", get_base_url());
	let response = apply_headers(Request::get(url)?).send()?;
	if response.status_code() >= 400 {
		bail!(
			"HTTP {} while fetching Ranobe chapter list",
			response.status_code()
		);
	}
	let response = response.get_json_owned::<DesuChaptersResponse>()?;
	response
		.chapters
		.ok_or(error!("Desu API не вернул список глав ранобэ"))
}

fn fetch_ranobe_chapters(book_id: &str) -> Result<Vec<Chapter>> {
	Ok(ranobe_chapters_from_api(fetch_ranobe_chapter_records(
		book_id,
	)?))
}

pub fn fetch_ranobe_chapter_list(slug: &str) -> Result<Vec<Chapter>> {
	let book_id = ranobe_id_from_slug(slug).ok_or(error!("Invalid ranobe id"))?;
	fetch_ranobe_chapters(book_id)
}

pub fn ranobe_chapter_id_from_deep_link(slug: &str, path: &str) -> Result<Option<String>> {
	let base = get_base_url();
	let Some(target_url) = ranobe_chapter_deep_link_url(&base, path, slug) else {
		return Ok(None);
	};
	let book_id = ranobe_id_from_slug(slug).ok_or(error!("Invalid ranobe id"))?;
	let chapters = fetch_ranobe_chapter_records(book_id)?;
	Ok(chapter_id_from_api_view_url(&chapters, &base, &target_url))
}

fn api_chapter_id(slug: &str, chapter: &Chapter) -> Result<String> {
	if let Ok(id) = chapter.key.parse::<i64>() {
		return Ok(id.to_string());
	}

	let book_id = ranobe_id_from_slug(slug).ok_or(error!("Invalid ranobe id"))?;
	let base = get_base_url();
	let target_url = chapter.url.as_deref().unwrap_or(chapter.key.as_str());
	let chapters = fetch_ranobe_chapter_records(book_id)?;
	chapter_id_from_api_view_url(&chapters, &base, target_url)
		.ok_or(error!("Не удалось сопоставить главу Desu с её API id"))
}

fn fetch_html(url: &str) -> Result<Document> {
	require_login()?;
	let response = apply_headers(Request::get(url)?.authed()).send()?;
	if response.status_code() == 401 {
		bail!("Требуется вход в аккаунт Desu");
	}
	if response.status_code() >= 400 {
		bail!("HTTP {}", response.status_code());
	}
	Ok(response.get_html()?)
}

fn cover_from_style(style: &str) -> Option<String> {
	let start = style.find("url(")?;
	let rest = &style[start + 4..];
	let rest = rest.trim_start_matches(['\'', '"']);
	let end = rest.find(['\'', '"', ')'])?;
	let url = rest[..end].trim();
	(!url.is_empty()).then(|| url.into())
}

fn parse_catalog_item(li: &Element) -> Option<Manga> {
	let link = li.select_first("h3 a.animeTitle, h3 a")?;
	let href = link.attr("href")?;
	let slug = ranobe_slug(&href)?;
	let eng = link.text()?.trim().to_string();
	let russian = li
		.select_first(".dimmed.oTitle span, .dimmed.oTitle")
		.and_then(|el| el.text())
		.map(|s| s.trim().to_string())
		.filter(|s| !s.is_empty());
	let title = if eng_title() {
		eng
	} else {
		russian.clone().unwrap_or(eng)
	};
	let cover = li
		.select_first("span.img")
		.and_then(|el| el.attr("style"))
		.and_then(|s| cover_from_style(&s))
		.map(|url| rewrite_media_url(&url))
		.or_else(|| {
			slug.rsplit_once('.')
				.map(|(_, id)| ranobe_cover_preview(id))
		});
	let url = Some(format!("{}/ranobe/{slug}/", get_base_url()));
	Some(Manga {
		key: ranobe_key(&slug),
		title,
		cover,
		url,
		viewer: Viewer::LeftToRight,
		..Default::default()
	})
}

fn is_confirmed_ranobe_status(status: &str) -> bool {
	matches!(status, "ongoing" | "released" | "continued" | "completed")
}

fn ranobe_catalog_query(
	page: i32,
	query: Option<&str>,
	filters: Vec<FilterValue>,
) -> QueryParameters {
	let mut qs = QueryParameters::new();
	qs.push("page", Some(page.to_string().as_str()));
	if let Some(query) = query.filter(|query| !query.is_empty()) {
		qs.push("search", Some(query));
	}

	let mut order = "updated";
	let mut genres = Vec::new();
	let mut statuses = Vec::new();
	for filter in filters {
		match filter {
			FilterValue::Sort { index, .. } => {
				order = match index {
					0 => "id",
					1 => "name",
					2 => "popular",
					_ => order,
				};
			}
			FilterValue::MultiSelect { id, included, .. } if id == "ranobe_genres" => {
				genres = included;
			}
			FilterValue::MultiSelect { id, included, .. } if id == "ranobe_status" => {
				statuses = included
					.into_iter()
					.filter(|status| is_confirmed_ranobe_status(status))
					.collect();
			}
			_ => {}
		}
	}

	qs.push("order_by", Some(order));
	if !statuses.is_empty() {
		qs.push("status", Some(&statuses.join(",")));
	}
	if !genres.is_empty() {
		qs.push("genres", Some(&genres.join(",")));
	}
	qs
}

pub fn search_ranobe(
	query: Option<String>,
	page: i32,
	filters: Vec<FilterValue>,
) -> Result<MangaPageResult> {
	let qs = ranobe_catalog_query(page, query.as_deref(), filters);
	let url = format!("{}/ranobe/?{qs}", get_base_url());
	let html = fetch_html(&url)?;
	let entries: Vec<Manga> = html
		.select("li.memberListItem")
		.map(|els| els.filter_map(|li| parse_catalog_item(&li)).collect())
		.unwrap_or_default();
	let last_page = html
		.select_first(".PageNav")
		.and_then(|nav| nav.attr("data-last"))
		.and_then(|s| s.parse::<i32>().ok())
		.unwrap_or(1);
	let has_next_page =
		page < last_page || (last_page == 1 && entries.len() as i32 >= RANOBE_PAGE_SIZE);
	Ok(MangaPageResult {
		entries,
		has_next_page,
	})
}

fn parse_status(html: &Document) -> MangaStatus {
	let text = html
		.select_first("span.b-anime_status_tag")
		.and_then(|el| el.text())
		.unwrap_or_default()
		.to_lowercase();
	if text.contains("выход")
		|| text.contains("онгоинг")
		|| text.contains("ongoing")
		|| text.contains("перевод")
		|| text.contains("continued")
	{
		MangaStatus::Ongoing
	} else if text.contains("заверш") || text.contains("издан") || text.contains("complet")
	{
		MangaStatus::Completed
	} else {
		MangaStatus::Unknown
	}
}

fn parse_title_pair(html: &Document) -> (String, Option<String>) {
	if let Some(og) = html
		.select_first("meta[property='og:title']")
		.and_then(|el| el.attr("content"))
	{
		let og = og.trim().to_string();
		if let Some(h1) = html.select_first("h1").and_then(|el| el.text()) {
			let h1 = h1.trim().to_string();
			if let Some((eng, rus)) = h1.split_once(" / ") {
				return (eng.trim().into(), Some(rus.trim().into()));
			}
			return (h1, Some(og));
		}
		return (og, None);
	}
	let h1 = html
		.select_first("h1")
		.and_then(|el| el.text())
		.unwrap_or_default()
		.trim()
		.to_string();
	if let Some((eng, rus)) = h1.split_once(" / ") {
		(eng.trim().into(), Some(rus.trim().into()))
	} else {
		(h1, None)
	}
}

fn ranobe_authors(html: &Document) -> Vec<String> {
	html.select(".line")
		.map(|rows| {
			rows.filter_map(|row| {
				let label = row
					.select_first(".key")
					.and_then(|element| element.text())
					.unwrap_or_default()
					.to_lowercase();
				if !label.contains("автор") && !label.contains("author") {
					return None;
				}
				Some(
					row.select(".value a")
						.map(|links| {
							links
								.filter_map(|link| {
									link.text()
										.map(|name| name.trim().to_string())
										.filter(|name| !name.is_empty())
								})
								.collect::<Vec<_>>()
						})
						.unwrap_or_default(),
				)
			})
			.flatten()
			.collect()
		})
		.unwrap_or_default()
}

fn ranobe_tags(html: &Document) -> Vec<String> {
	html.select(".tagList a")
		.map(|elements| {
			elements
				.filter_map(|element| {
					element
						.text()
						.map(|text| text.trim().to_string())
						.filter(|text| !text.is_empty())
				})
				.collect()
		})
		.unwrap_or_default()
}

fn ranobe_content_rating(tags: &[String]) -> ContentRating {
	let has_any = |needles: &[&str]| {
		tags.iter().any(|tag| {
			let tag = tag.to_lowercase();
			needles.iter().any(|needle| tag.contains(needle))
		})
	};

	if has_any(&["хентай", "hentai", "18+", "nsfw", "эротика", "erotica"]) {
		ContentRating::NSFW
	} else if has_any(&["этти", "ecchi", "suggestive"]) {
		ContentRating::Suggestive
	} else if tags.is_empty() {
		ContentRating::Unknown
	} else {
		ContentRating::Safe
	}
}

pub fn fetch_ranobe(slug: &str) -> Result<Manga> {
	let base = get_base_url();
	let url = format!("{base}/ranobe/{slug}/");
	let html = fetch_html(&url)?;
	let (eng, russian) = parse_title_pair(&html);
	let mut manga = Manga {
		key: ranobe_key(slug),
		url: Some(url),
		viewer: Viewer::LeftToRight,
		title: if eng_title() {
			eng
		} else {
			russian.unwrap_or(eng)
		},
		..Default::default()
	};
	manga.cover = html
		.select_first("img[src*='ranobe/covers']")
		.and_then(|el| el.attr("abs:src"))
		.map(|url| rewrite_media_url(&url))
		.or_else(|| {
			slug.rsplit_once('.')
				.map(|(_, id)| ranobe_cover_preview(id))
		});
	manga.description = html
		.select_first("[itemprop=description]")
		.and_then(|el| el.text())
		.map(|s| s.trim().to_string())
		.filter(|s| !s.is_empty());
	manga.status = parse_status(&html);
	let authors = ranobe_authors(&html);
	manga.authors = (!authors.is_empty()).then_some(authors);
	let tags = ranobe_tags(&html);
	manga.content_rating = ranobe_content_rating(&tags);
	manga.tags = (!tags.is_empty()).then_some(tags);
	Ok(manga)
}

pub fn fetch_ranobe_chapter_pages(slug: &str, chapter: &Chapter) -> Result<Vec<Page>> {
	require_login()?;
	let book_id = ranobe_id_from_slug(slug).ok_or(error!("Invalid ranobe id"))?;
	let chapter_id = api_chapter_id(slug, chapter)?;
	let url = format!(
		"{}/api/ranobe/{book_id}/chapters/{chapter_id}",
		get_base_url()
	);
	let base = get_base_url();
	let title_url = format!("{base}/ranobe/{slug}/");
	let referer = chapter.url.as_deref().unwrap_or(title_url.as_str());
	let response = apply_headers(Request::get(url)?)
		.header("Referer", referer)
		.send()?;
	if response.status_code() == 401 {
		bail!("Требуется вход в аккаунт Desu");
	}
	if response.status_code() >= 400 {
		bail!(
			"HTTP {} while fetching Ranobe chapter",
			response.status_code()
		);
	}
	let response = response.get_json_owned::<RanobeChapterResponse>()?;
	let chapter = response
		.chapter
		.ok_or(error!("Desu API не вернул данные главы"))?;
	chapter_pages_from_content(&chapter.content.unwrap_or_default(), &base)
}

#[cfg(test)]
mod tests {
	use super::{
		RanobeContentBlock, chapter_id_from_api_view_url, chapter_pages_from_content,
		chapter_text_from_content, parse_status, ranobe_authors, ranobe_catalog_query,
		ranobe_chapter_deep_link_url, ranobe_chapters_from_api, ranobe_content_rating,
		ranobe_id_from_slug, ranobe_tags,
	};
	use crate::models::DesuChapter;
	use aidoku::alloc::{string::ToString, vec};
	use aidoku::{ContentRating, FilterValue, PageContent, imports::html::Html};
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn extracts_api_book_id_from_ranobe_slug() {
		assert_eq!(ranobe_id_from_slug("example-title.51"), Some("51"));
	}

	#[aidoku_test]
	fn maps_ranobe_api_chapters_and_skips_unreadable_entries() {
		let chapters = vec![
			DesuChapter {
				id: 54507,
				manga_id: None,
				volume: Some("35".into()),
				number: Some("0.1".into()),
				title: Some("Том 35. Глава 0.1".into()),
				publish_date: Some(1_700_000_000),
				view_url: Some("https://desu.uno/ranobe/sample.51/vol35/ch0.1/rus".into()),
				is_readable: Some(true),
			},
			DesuChapter {
				id: 54508,
				manga_id: None,
				volume: Some("35".into()),
				number: Some("0.2".into()),
				title: Some("Том 35. Глава 0.2".into()),
				publish_date: None,
				view_url: Some("https://desu.uno/ranobe/sample.51/vol35/ch0.2/rus".into()),
				is_readable: Some(false),
			},
			DesuChapter {
				id: 54509,
				manga_id: None,
				volume: None,
				number: None,
				title: None,
				publish_date: None,
				view_url: Some("https://desu.uno/ranobe/sample.51/bonus/rus".into()),
				is_readable: None,
			},
		];
		let mapped = ranobe_chapters_from_api(chapters);

		assert_eq!(mapped.len(), 2);
		assert_eq!(mapped[0].key, "54507");
		assert_eq!(mapped[0].volume_number, Some(35.0));
		assert_eq!(mapped[0].chapter_number, Some(0.1));
		assert_eq!(mapped[0].date_uploaded, Some(1_700_000_000));
		assert_eq!(mapped[1].key, "54509");
	}

	#[aidoku_test]
	fn identifies_chapter_deep_link_and_keeps_query_out_of_match() {
		assert_eq!(
			ranobe_chapter_deep_link_url(
				"https://desu.uno",
				"ranobe/sample.51/vol35/ch0.1/rus/?from=external",
				"sample.51"
			)
			.as_deref(),
			Some("https://desu.uno/ranobe/sample.51/vol35/ch0.1/rus/")
		);
		assert_eq!(
			ranobe_chapter_deep_link_url("https://desu.uno", "ranobe/sample.51/", "sample.51"),
			None
		);
		assert_eq!(
			ranobe_chapter_deep_link_url(
				"https://desu.uno",
				"ranobe/another.51/vol1/ch1/rus",
				"sample.51"
			),
			None
		);
	}

	#[aidoku_test]
	fn resolves_legacy_ranobe_chapter_urls_against_api_view_urls() {
		let chapters = vec![DesuChapter {
			id: 54507,
			manga_id: None,
			volume: Some("35".into()),
			number: Some("0.1".into()),
			title: None,
			publish_date: None,
			view_url: Some("/ranobe/sample.51/vol35/ch0.1/rus".into()),
			is_readable: Some(true),
		}];

		assert_eq!(
			chapter_id_from_api_view_url(
				&chapters,
				"https://desu.uno",
				"https://desu.uno/ranobe/sample.51/vol35/ch0.1/rus/?from=legacy"
			),
			Some("54507".into())
		);
	}

	#[aidoku_test]
	fn refuses_to_match_legacy_chapters_from_other_hosts() {
		let chapters = vec![DesuChapter {
			id: 54507,
			manga_id: None,
			volume: None,
			number: None,
			title: None,
			publish_date: None,
			view_url: Some("https://desu.uno/ranobe/sample.51/vol35/ch0.1/rus".into()),
			is_readable: Some(true),
		}];

		assert_eq!(
			chapter_id_from_api_view_url(
				&chapters,
				"https://desu.uno",
				"https://evil.example/ranobe/sample.51/vol35/ch0.1/rus"
			),
			None
		);
	}

	#[aidoku_test]
	fn combines_text_blocks_in_order_and_skips_images() {
		let content = [
			RanobeContentBlock {
				content_type: Some("image".into()),
				html: None,
				url: Some("https://img2.desu.uno/ranobe/illustration.jpg".into()),
			},
			RanobeContentBlock {
				content_type: Some("text".into()),
				html: Some("<p>Line one.</p><p>Line two.</p>".into()),
				url: None,
			},
			RanobeContentBlock {
				content_type: Some("image".into()),
				html: None,
				url: Some("https://img2.desu.uno/ranobe/second-illustration.jpg".into()),
			},
			RanobeContentBlock {
				content_type: Some("text".into()),
				html: Some("<p>Line three.</p>".into()),
				url: None,
			},
		];

		assert_eq!(
			chapter_text_from_content(&content).unwrap().as_deref(),
			Some("Line one.\n\nLine two.\n\nLine three.")
		);
	}

	#[aidoku_test]
	fn extracts_text_html_blocks_without_paragraph_wrappers() {
		let content = [RanobeContentBlock {
			content_type: None,
			html: Some("<div>Opening<br>line</div>".into()),
			url: None,
		}];

		assert_eq!(
			chapter_text_from_content(&content).unwrap().as_deref(),
			Some("Opening\nline")
		);
	}

	#[aidoku_test]
	fn preserves_direct_text_around_paragraphs() {
		let content = [RanobeContentBlock {
			content_type: Some("text".into()),
			html: Some("Opening note.<p>Paragraph.</p>Closing note.".into()),
			url: None,
		}];

		assert_eq!(
			chapter_text_from_content(&content).unwrap().as_deref(),
			Some("Opening note.\n\nParagraph.\n\nClosing note.")
		);
	}

	#[aidoku_test]
	fn preserves_non_paragraph_blocks_between_paragraphs() {
		let content = [RanobeContentBlock {
			content_type: None,
			html: Some("<div><p>First.</p><h2>Interlude</h2><p>Last.</p></div>".into()),
			url: None,
		}];

		assert_eq!(
			chapter_text_from_content(&content).unwrap().as_deref(),
			Some("First.\n\nInterlude\n\nLast.")
		);
	}

	#[aidoku_test]
	fn preserves_line_breaks_inside_api_paragraphs() {
		let content = [RanobeContentBlock {
			content_type: None,
			html: Some("<p>First line<br>Second <b>bold</b>.</p>".into()),
			url: None,
		}];

		assert_eq!(
			chapter_text_from_content(&content).unwrap().as_deref(),
			Some("First line\nSecond **bold**.")
		);
	}

	#[aidoku_test]
	fn reports_no_text_for_image_only_chapters() {
		let content = [RanobeContentBlock {
			content_type: None,
			html: None,
			url: Some("https://img2.desu.uno/ranobe/illustration.jpg".into()),
		}];

		assert_eq!(chapter_text_from_content(&content).unwrap(), None);
	}

	#[aidoku_test]
	fn image_blocks_with_html_remain_image_pages() {
		let content = [RanobeContentBlock {
			content_type: Some("image".into()),
			html: Some("<p>not reader text</p>".into()),
			url: Some("https://img2.desu.uno/ranobe/image.jpg".into()),
		}];
		let pages = chapter_pages_from_content(&content, "https://desu.uno").unwrap();

		assert_eq!(pages.len(), 1);
		assert_eq!(
			pages[0].content,
			PageContent::url("https://img2.desu.uno/ranobe/image.jpg")
		);
	}

	#[aidoku_test]
	fn image_only_chapters_keep_their_image_pages() {
		let content = [
			RanobeContentBlock {
				content_type: Some("image".into()),
				html: None,
				url: Some("https://img2.desu.uno/ranobe/first.jpg?v=1".into()),
			},
			RanobeContentBlock {
				content_type: Some("image".into()),
				html: None,
				url: Some("https://img2.desu.uno/ranobe/second.jpg?v=2".into()),
			},
		];
		let pages = chapter_pages_from_content(&content, "https://desu.uno").unwrap();

		assert_eq!(pages.len(), 2);
		assert_eq!(
			pages[0].content,
			PageContent::url("https://img2.desu.uno/ranobe/first.jpg?v=1")
		);
		assert_eq!(
			pages[1].content,
			PageContent::url("https://img2.desu.uno/ranobe/second.jpg?v=2")
		);
	}

	#[aidoku_test]
	fn mixed_chapters_use_one_text_page_and_skip_all_images() {
		let content = [
			RanobeContentBlock {
				content_type: Some("image".into()),
				html: None,
				url: Some("https://img2.desu.uno/ranobe/illustration.jpg".into()),
			},
			RanobeContentBlock {
				content_type: Some("text".into()),
				html: Some("<p>First paragraph.</p><p>Second paragraph.</p>".into()),
				url: None,
			},
			RanobeContentBlock {
				content_type: Some("image".into()),
				html: None,
				url: Some("https://img2.desu.uno/ranobe/another-illustration.jpg".into()),
			},
			RanobeContentBlock {
				content_type: Some("text".into()),
				html: Some("<p>Third paragraph.</p>".into()),
				url: None,
			},
		];
		let pages = chapter_pages_from_content(&content, "https://desu.uno").unwrap();

		assert_eq!(pages.len(), 1);
		assert_eq!(
			pages[0].content,
			PageContent::text("First paragraph.\n\nSecond paragraph.\n\nThird paragraph.")
		);
	}

	#[aidoku_test]
	fn maps_ranobe_translation_in_progress_to_ongoing() {
		let document =
			Html::parse_fragment(r#"<span class="b-anime_status_tag">Переводится</span>"#).unwrap();

		assert!(matches!(
			parse_status(&document),
			aidoku::MangaStatus::Ongoing
		));
	}

	#[aidoku_test]
	fn parses_only_author_row_not_translators() {
		let document = Html::parse_fragment(
			r#"<div class="line"><div class="key">Авторы:</div><div class="value"><ul class="translators"><li><a>KINUGASA Shougo</a></li><li><a>TOMOSE Shunsaku</a></li></ul></div></div><div class="line"><div class="key">Переводчики:</div><div class="value"><ul class="translators"><li><a>RanobeList</a></li></ul></div></div>"#,
		)
		.unwrap();

		assert_eq!(
			ranobe_authors(&document),
			vec!["KINUGASA Shougo".to_string(), "TOMOSE Shunsaku".to_string()]
		);
	}

	#[aidoku_test]
	fn parses_ranobe_tags_from_the_tag_list() {
		let document = Html::parse_fragment(
			r#"<ul class="tagList"><li><a>Драма</a></li><li><a>Этти</a></li></ul>"#,
		)
		.unwrap();
		let tags = ranobe_tags(&document);

		assert_eq!(tags.len(), 2);
		assert_eq!(tags[0], "Драма");
		assert_eq!(tags[1], "Этти");
	}

	#[aidoku_test]
	fn derives_ranobe_content_rating_from_explicit_tags() {
		assert_eq!(
			ranobe_content_rating(&["Фэнтези".into()]),
			ContentRating::Safe
		);
		assert_eq!(
			ranobe_content_rating(&["Этти".into()]),
			ContentRating::Suggestive
		);
		assert_eq!(
			ranobe_content_rating(&["Хентай".into()]),
			ContentRating::NSFW
		);
		assert_eq!(ranobe_content_rating(&[]), ContentRating::Unknown);
	}

	#[aidoku_test]
	fn builds_ranobe_catalog_url_and_ignores_unconfirmed_status_and_genre_values() {
		let filters = vec![
			FilterValue::Sort {
				id: "order".into(),
				index: 2,
				ascending: false,
			},
			FilterValue::MultiSelect {
				id: "ranobe_status".into(),
				included: vec!["ongoing".into(), "mylist".into()],
				excluded: vec![],
			},
			FilterValue::MultiSelect {
				id: "ranobe_genres".into(),
				included: vec!["100-Dementia".into()],
				excluded: vec!["104-Erotica".into()],
			},
		];
		let query = ranobe_catalog_query(2, Some("Classroom"), filters).to_string();

		assert_eq!(
			query,
			"page=2&search=Classroom&order_by=popular&status=ongoing&genres=100-Dementia"
		);
	}

	#[aidoku_test]
	fn parses_api_reader_fragment_without_live_network() {
		let document = Html::parse_fragment("<p>Sample paragraph.</p>").unwrap();
		assert_eq!(
			document.select("p").unwrap().text().as_deref(),
			Some("Sample paragraph.")
		);
	}
}
