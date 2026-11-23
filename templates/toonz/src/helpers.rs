use crate::{
	Params,
	models::{ChapterItem, ChapterList, FeaturedSeries, Genre},
	settings,
};
use aidoku::{
	ContentRating, Manga, MangaPageResult, MangaStatus, Page, PageContent, Result, Viewer,
	alloc::{String, Vec, borrow::Cow, collections::BTreeSet, format, string::ToString, vec},
	helpers::uri::{QueryParameters, encode_uri_component},
	imports::{
		html::{Document, Element, ElementList},
		net::{Request, Response},
	},
	prelude::*,
};
use serde::Serialize;

const TYPE_PAGES: [&str; 5] = [
	"browse",
	"top-rated",
	"most-popular",
	"trending",
	"most-favorited",
];

pub const SORTS: [&str; 4] = ["latest", "popular", "rating", "a-z"];

// The site reads no types as every type; an unknown one matches nothing.
pub fn narrowed(types: Vec<String>, count: usize) -> Vec<String> {
	if types.is_empty() {
		vec!["none".into()]
	} else if types.len() < count {
		types
	} else {
		Vec::new()
	}
}

pub fn check_response(response: Response) -> Result<Response> {
	let status = response.status_code();
	if !(200..300).contains(&status) {
		bail!("The site answered HTTP {status}");
	}
	Ok(response)
}

#[derive(Default, Serialize)]
pub struct Content {
	pub rating: String,
	pub types: Vec<String>,
	#[serde(rename = "blockedGenres")]
	pub blocked: Vec<String>,
}

impl Content {
	pub fn new(params: &Params) -> Self {
		Self {
			rating: settings::content_rating(params.rating),
			types: settings::types(params.type_count),
			blocked: settings::excluded_genres(),
		}
	}

	pub fn request(&self, url: &str) -> Result<Request> {
		let prefs = serde_json::to_string(self)?;
		Ok(Request::get(url)?.header(
			"Cookie",
			&format!("reader_prefs={}", encode_uri_component(prefs)),
		))
	}
}

#[derive(Default)]
pub struct SearchParams {
	pub query: Option<String>,
	pub sort: &'static str,
	pub status: Option<String>,
	pub author: Option<String>,
	pub genres: Vec<String>,
	pub excluded_genres: Vec<String>,
	pub content: Content,
}

impl SearchParams {
	pub fn new(params: &Params, query: Option<String>) -> Self {
		Self {
			query,
			sort: SORTS[0],
			content: Content::new(params),
			..Default::default()
		}
	}

	pub fn page(&self, params: &Params, page: i32) -> Result<MangaPageResult> {
		if self.content.types.as_slice() == ["none"] {
			return Ok(MangaPageResult::default());
		}
		let mut query = QueryParameters::new();
		if self.query.is_some() {
			query.push("search", self.query.as_deref());
		}
		query.push_encoded("sort", Some(self.sort));
		if self.status.is_some() {
			query.push_encoded("status", self.status.as_deref());
		}
		if let Some(author) = &self.author {
			query.push("author", Some(&slugify(author)));
		}
		if !self.genres.is_empty() {
			query.push("genres", Some(&self.genres.join(",")));
		}
		if !self.excluded_genres.is_empty() {
			query.push("excludeGenres", Some(&self.excluded_genres.join(",")));
		}
		query.push_encoded("page", Some(&page.to_string()));
		let url = format!("{}/{}?{query}", params.base_url, params.catalog);
		let document = check_response(self.content.request(&url)?.send()?)?.get_html()?;
		Ok(MangaPageResult {
			entries: cards(params, document.select("img[data-cover]")),
			has_next_page: document
				.select_first("a[aria-label='Go to next page'][href]")
				.is_some(),
		})
	}
}

pub fn parse_series_path<'a>(params: &Params, path: &'a str) -> Option<(&'a str, Option<&'a str>)> {
	let mut parts = path
		.strip_prefix('/')?
		.split(['?', '#'])
		.next()?
		.trim_end_matches('/')
		.split('/');
	let kind = parts.next()?;
	let slug = parts.next().filter(|slug| !slug.is_empty())?;
	if !params.series_types.contains(&kind) || TYPE_PAGES.contains(&slug) {
		return None;
	}
	let chapter = match (parts.next(), parts.next(), parts.next()) {
		(None, None, None) => None,
		(Some("chapter"), Some(chapter), None) if !chapter.is_empty() => Some(chapter),
		_ => return None,
	};
	Some((slug, chapter))
}

// LewdManhwa's series link is an empty overlay beside the cover, not around it.
pub fn card(params: &Params, image: &Element) -> Option<(Element, Manga)> {
	let mut node = image.parent()?;
	for _ in 0..5 {
		let key = node
			.attr("href")
			.into_iter()
			.chain(
				node.select("a[href]")
					.into_iter()
					.flatten()
					.filter_map(|link| link.attr("href")),
			)
			.find_map(|href| parse_series_path(params, &href).map(|(key, _)| String::from(key)));
		if let Some(key) = key {
			let title = node
				.select_first("h3")
				.and_then(|heading| heading.text())
				.filter(|title| !title.is_empty())
				.or_else(|| {
					let title = image.attr("alt")?;
					let title = title.trim();
					(!title.is_empty()).then(|| title.into())
				})?;
			let manga = Manga {
				key,
				title,
				cover: image.attr("abs:src"),
				content_rating: rating(image.has_attr("data-nsfw"), &[]),
				..Default::default()
			};
			return Some((node, manga));
		}
		node = node.parent()?;
	}
	None
}

pub fn cards(params: &Params, images: Option<ElementList>) -> Vec<Manga> {
	let mut seen = BTreeSet::new();
	images
		.into_iter()
		.flatten()
		.filter_map(|image| {
			let (_, manga) = card(params, &image)?;
			seen.insert(manga.key.clone()).then_some(manga)
		})
		.collect()
}

// `nsfw` is the site's blur flag on erotica and explicit covers.
pub fn rating(nsfw: bool, tags: &[String]) -> ContentRating {
	let has = |names: &[&str]| tags.iter().any(|tag| names.contains(&tag.as_str()));
	if nsfw || has(&["Adult", "Erotica", "Hentai", "Smut"]) {
		ContentRating::NSFW
	} else if has(&["Ecchi", "Mature"]) {
		ContentRating::Suggestive
	} else if tags.is_empty() {
		ContentRating::Unknown
	} else {
		ContentRating::Safe
	}
}

pub fn status(text: &str) -> Option<MangaStatus> {
	Some(match text.to_lowercase().as_str() {
		"ongoing" => MangaStatus::Ongoing,
		"completed" => MangaStatus::Completed,
		"hiatus" => MangaStatus::Hiatus,
		"cancelled" => MangaStatus::Cancelled,
		_ => return None,
	})
}

fn viewer(
	kind: Option<&str>,
	origin: Option<&str>,
	tags: &[String],
	badges: &[String],
	alt_titles: &[String],
) -> Viewer {
	if tags.iter().any(|tag| {
		[
			"Long Strip",
			"Longstrip",
			"Webtoon",
			"Webtoons",
			"Manhwa",
			"Manhua",
		]
		.iter()
		.any(|name| tag.eq_ignore_ascii_case(name))
	}) {
		return Viewer::Webtoon;
	}
	let origin = origin.map(str::to_lowercase);
	if matches!(
		origin.as_deref(),
		Some(
			"south korea"
				| "korea" | "korean"
				| "china" | "chinese"
				| "chinese (hong kong)"
				| "taiwan" | "hong kong"
		)
	) {
		return Viewer::Webtoon;
	}
	// Mistagged Korean manhwas can list Japan as their origin while keeping a Hangul alt title.
	if alt_titles.iter().any(|title| {
		title
			.chars()
			.any(|c| ('\u{AC00}'..='\u{D7A3}').contains(&c))
	}) {
		return Viewer::Webtoon;
	}
	kind.into_iter()
		.chain(badges.iter().map(String::as_str))
		.find_map(|text| match text.to_lowercase().as_str() {
			"manhwa" | "manhua" | "webtoon" => Some(Viewer::Webtoon),
			"manga" => Some(Viewer::RightToLeft),
			"comic" | "western" => Some(Viewer::LeftToRight),
			_ => None,
		})
		.unwrap_or_default()
}

pub fn age(text: &str) -> Option<i64> {
	if text == "just now" {
		return Some(0);
	}
	let amount = text.strip_suffix(" ago")?;
	let split = amount.find(|c: char| !c.is_ascii_digit())?;
	let (number, unit) = amount.split_at(split);
	let unit = match unit {
		"m" => 60,
		"h" => 3600,
		"d" => 86400,
		"w" => 604_800,
		"mo" => 2_592_000,
		"y" => 31_536_000,
		_ => return None,
	};
	number.parse::<i64>().ok()?.checked_mul(unit)
}

pub fn slugify(name: &str) -> String {
	let mut slug = String::new();
	for c in name.trim().to_lowercase().chars() {
		if c.is_ascii_alphanumeric() {
			slug.push(c);
		} else if (c.is_whitespace() || c == '-' || c == '_') && !slug.ends_with('-') {
			slug.push('-');
		}
	}
	slug.trim_matches('-').into()
}

pub fn chapter_url(params: &Params, manga_key: &str, key: &str) -> String {
	format!(
		"{}/{}/{manga_key}/chapter/{key}",
		params.base_url, params.series_path
	)
}

pub fn series_page(params: &Params, key: &str) -> Result<Document> {
	let url = format!("{}/{}/{key}", params.base_url, params.series_path);
	Ok(check_response(Request::get(url)?.send()?)?.get_html()?)
}

pub(crate) fn comic_id(page: &Document) -> Result<String> {
	page.select_first("[data-comic-id]")
		.and_then(|marker| marker.attr("data-comic-id"))
		.filter(|id| !id.trim().is_empty())
		.ok_or_else(|| error!("Series not found"))
}

type GenreOptions = (Vec<Cow<'static, str>>, Vec<Cow<'static, str>>);

pub fn genre_options(params: &Params) -> Result<GenreOptions> {
	let url = format!("{}/api/genres", params.base_url);
	let genres: Vec<Genre> = check_response(Request::get(url)?.send()?)?.get_json_owned()?;
	Ok(genres
		.into_iter()
		.filter(|genre| !genre.name.trim().is_empty() && !genre.slug.trim().is_empty())
		.map(|genre| (genre.name.into(), genre.slug.into()))
		.unzip())
}

pub fn chapter_items(params: &Params, page: &Document) -> Result<Vec<ChapterItem>> {
	let url = format!(
		"{}/api/comics/{}/chapters",
		params.base_url,
		comic_id(page)?
	);
	Ok(check_response(Request::get(url)?.send()?)?
		.get_json_owned::<ChapterList>()?
		.chapters)
}

pub fn reader_page(params: &Params, manga_key: &str, chapter_key: &str) -> Result<Option<String>> {
	let response = Request::get(chapter_url(params, manga_key, chapter_key))?
		.header("RSC", "1")
		.send()?;
	if response.status_code() == 404 {
		return Ok(None);
	}
	Ok(Some(check_response(response)?.get_string()?))
}

pub fn upload_page(base_url: &str, filename: &str) -> Result<Page> {
	let filename = filename.trim().trim_start_matches('/');
	if filename.is_empty() {
		bail!("This chapter has missing pages");
	}
	Ok(Page {
		content: PageContent::url(format!("{base_url}/uploads/{filename}")),
		..Default::default()
	})
}

pub fn cover_url(params: &Params, featured: Option<&FeaturedSeries>) -> Option<String> {
	let path = featured?.cover.as_deref()?.trim();
	(!path.is_empty()).then(|| format!("{}/uploads/{path}", params.base_url))
}

fn texts(list: Option<ElementList>) -> impl Iterator<Item = String> {
	list.into_iter()
		.flatten()
		.filter_map(|element| element.text())
}

pub fn series_summary(page: &Document) -> Result<(String, Option<String>)> {
	// Next.js can stream the series content outside <main>.
	let title = page
		.select_first("h1")
		.and_then(|heading| heading.text())
		.filter(|title| !title.is_empty())
		.ok_or_else(|| error!("Series not found"))?;
	let cover = page
		.select_first("meta[property='og:image']")
		.and_then(|meta| meta.attr("content"));
	Ok((title, cover))
}

pub fn details(page: &Document, manga: &mut Manga) -> Result<()> {
	(manga.title, manga.cover) = series_summary(page)?;
	let nsfw = page
		.select_first("img[data-cover]")
		.is_some_and(|cover| cover.has_attr("data-nsfw"));
	let tags = tags(page.select("a[href^='/genre/']"));
	manga.description = page
		.select_first("p.whitespace-pre-line")
		.and_then(|text| text.untrimmed_text())
		.and_then(|text| {
			let text = text.trim();
			(!text.is_empty()).then(|| text.into())
		});
	manga.url = page
		.select_first("link[rel=canonical]")
		.and_then(|link| link.attr("href"))
		.filter(|url| !url.is_empty());
	manga.content_rating = rating(nsfw, &tags);

	// One layout lists these in a table, the other as badges and author links.
	let mut status_text = None;
	let mut type_text = None;
	let mut origin_text = None;
	let mut alt_titles: Vec<String> = texts(page.select("h1 ~ p")).collect();
	let mut author_links = page.select("a[href^='/author/']");
	for row in page.select("dl > div").into_iter().flatten() {
		let (Some(label), Some(value)) = (
			row.select_first("dt").and_then(|label| label.text()),
			row.select_first("dd"),
		) else {
			continue;
		};
		match label.as_str() {
			"Status" => status_text = value.text(),
			"Type" => type_text = value.text(),
			"Origin" => origin_text = value.text(),
			"Original title" => alt_titles.extend(value.text()),
			"Author" | "Authors" => author_links = value.select("a"),
			_ => {}
		}
	}
	let badges: Vec<String> = texts(page.select("[data-slot=badge]")).collect();
	manga.status = status_text
		.as_deref()
		.and_then(status)
		.or_else(|| badges.iter().find_map(|badge| status(badge)))
		.unwrap_or_default();
	manga.viewer = viewer(
		type_text.as_deref(),
		origin_text.as_deref(),
		&tags,
		&badges,
		&alt_titles,
	);
	manga.tags = Some(tags).filter(|tags| !tags.is_empty());
	let mut authors: Vec<String> = Vec::new();
	for name in texts(author_links).filter(|name| !name.is_empty()) {
		if !authors.contains(&name) {
			authors.push(name);
		}
	}
	manga.authors = Some(authors).filter(|authors| !authors.is_empty());
	Ok(())
}

pub fn tags(elements: Option<ElementList>) -> Vec<String> {
	let mut tags: Vec<_> = texts(elements).filter(|tag| !tag.is_empty()).collect();
	tags.sort_unstable();
	tags.dedup();
	tags
}
