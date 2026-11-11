use crate::{
	Params,
	helpers::{
		background_image, chapter_from_link, content_rating, fetch, labelled, log_response,
		manga_from_link, names, novel_text, series_key, status, viewer,
	},
	home::parse_home,
};
use aidoku::{
	Chapter, DeepLinkResult, FilterValue, HomeLayout, Manga, MangaPageResult, Page, PageContent,
	Result,
	alloc::{String, Vec, vec},
	helpers::string::{PlainText, StripPrefixOrSelf},
	imports::html::{Document, Html},
	prelude::*,
};

pub trait Impl {
	fn new() -> Self;

	fn params(&self) -> Params;

	// The site ignores query parameters and filters its full catalogue in the browser.
	fn get_search_manga_list(
		&self,
		params: &Params,
		query: Option<String>,
		_page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let url = format!("{}/series/", params.base_url);
		let response = fetch(&url)?;
		let html = response.get_html()?;
		let result = self.parse_search(params, &html, query, filters);
		if result.is_err() {
			log_response(&response, &url);
		}
		result
	}

	fn parse_search(
		&self,
		params: &Params,
		html: &Document,
		query: Option<String>,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		if html.select_first("#searched_series_page").is_none() {
			bail!("Catalogue not found");
		}
		let mut genres = Vec::new();
		let mut types = Vec::new();
		let mut statuses = Vec::new();
		for filter in filters {
			if let FilterValue::MultiSelect { id, included, .. } = filter {
				match id.as_str() {
					"genre" => genres = included.iter().map(|genre| genre.to_lowercase()).collect(),
					"type" => types = included,
					"status" => statuses = included,
					_ => {}
				}
			}
		}
		let query = query.map(|query| query.to_lowercase());
		let pasted = query
			.as_deref()
			.and_then(|query| query.strip_prefix(params.base_url.as_ref()))
			.and_then(series_key);

		let entries = html
			.select("#searched_series_page > button")
			.into_iter()
			.flatten()
			.filter(|card| {
				let matches_query = match pasted.as_deref() {
					Some(path) => card
						.select_first("a[href]")
						.and_then(|link| link.attr("href"))
						.is_some_and(|href| {
							href.strip_prefix_or_self(&params.base_url)
								.trim_end_matches('/') == path.trim_end_matches('/')
						}),
					// The card's title also carries the series' alternative titles
					None => query.as_ref().is_none_or(|query| {
						card.attr("title")
							.is_some_and(|title| title.to_lowercase().contains(query))
					}),
				};
				// Genre names can contain commas, and the site encodes apostrophes as ___.
				let has_genres = genres.is_empty()
					|| card.attr("tags").is_some_and(|tags| {
						let tags: Vec<String> = tags
							.trim_matches(['[', ']', '"'])
							.split("\",\"")
							.map(|tag| tag.trim().replace("___", "'").to_lowercase())
							.collect();
						genres.iter().all(|genre| tags.contains(genre))
					});
				let has = |values: &[String], attr: &str| {
					values.is_empty()
						|| card.attr(attr).is_some_and(|value| {
							values
								.iter()
								.any(|wanted| wanted.eq_ignore_ascii_case(&value))
						})
				};
				matches_query
					&& has_genres && has(&types, "data-type")
					&& has(&statuses, "data-status")
			})
			.filter_map(|card| {
				manga_from_link(&card.select_first("a[href]")?, &card, &params.base_url)
			})
			.collect();
		Ok(MangaPageResult {
			entries,
			has_next_page: false,
		})
	}

	fn get_manga_update(
		&self,
		params: &Params,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{}{}", params.base_url, manga.key);
		let response = fetch(&url)?;
		let html = response.get_html()?;
		let Some(title) = html
			.select_first("div.grid > h1")
			.and_then(|h1| h1.text())
			.filter(|title| !title.trim().is_empty())
		else {
			log_response(&response, &url);
			bail!("Series not found");
		};

		if needs_details {
			manga.title = title;
			manga.cover = html
				.select_first("[style*=photoURL]")
				.and_then(|cover| background_image(&cover))
				.or(manga.cover);
			manga.description = html
				.select_first("#expand_content, div.overflow-hidden > p[style*='white-space']")
				.and_then(|description| description.text())
				.map(|description| description.trim().escape_markdown())
				.filter(|description| !description.is_empty());
			manga.authors = labelled(&html, "Author").and_then(|author| names(&author));
			manga.artists = labelled(&html, "Artist").and_then(|artist| names(&artist));
			manga.status = labelled(&html, "Status")
				.map(|text| status(&text))
				.unwrap_or_default();
			let tags: Vec<String> = html
				.select("a[href*='genre=']")
				.into_iter()
				.flatten()
				.filter_map(|link| link.text())
				.map(|tag| tag.trim_matches([',', ' ']).into())
				.filter(|tag: &String| !tag.is_empty())
				.collect();
			// Keywords retain adult genres omitted from the visible tag links.
			let keywords = html
				.select_first("meta[name=keywords]")
				.and_then(|meta| meta.attr("content"));
			manga.content_rating = content_rating(
				tags.iter().map(String::as_str).chain(
					keywords
						.iter()
						.flat_map(|words| words.split(',').map(str::trim)),
				),
			);
			manga.viewer = labelled(&html, "Type")
				.map(|kind| viewer(&kind))
				.unwrap_or_default();
			manga.tags = (!tags.is_empty()).then_some(tags);
			manga.url = Some(url);
		}

		if needs_chapters {
			manga.chapters = html.select("#chapters > a").map(|rows| {
				rows.filter_map(|row| chapter_from_link(&row, &params.base_url))
					.collect()
			});
		}

		Ok(manga)
	}

	fn get_page_list(&self, params: &Params, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let response = fetch(&format!("{}{}", params.base_url, chapter.key))?.get_string()?;
		let html = Html::parse(&response)?;
		if let Some(reader) = html.select_first("div.tiptap.novel-reader") {
			let Some(text) = novel_text(&reader) else {
				bail!("No text found");
			};
			return Ok(vec![Page {
				content: PageContent::text(text),
				..Default::default()
			}]);
		}
		let template = response
			.split_once("realUrl")
			.and_then(|(_, rest)| rest.split_once('`'))
			.and_then(|(_, rest)| rest.split_once('`'))
			.map(|(template, _)| template)
			.ok_or_else(|| error!("No image host found"))?;
		let pages = html
			.select("#pages > img")
			.into_iter()
			.flatten()
			.map(|image| {
				let uid = image
					.attr("uid")
					.filter(|uid| !uid.is_empty())
					.ok_or_else(|| error!("A page has no image id"))?;
				Ok(Page {
					content: PageContent::url(template.replace("${uid}", &uid)),
					..Default::default()
				})
			})
			.collect::<Result<Vec<Page>>>()?;
		if pages.is_empty() {
			bail!("No pages found");
		}
		Ok(pages)
	}

	fn get_home(&self, params: &Params) -> Result<HomeLayout> {
		let html = fetch(&params.base_url)?.get_html()?;
		Ok(parse_home(params, &html))
	}

	fn handle_deep_link(&self, params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url.strip_prefix_or_self(&params.base_url);
		let path = path.split_once(['?', '#']).map_or(path, |(path, _)| path);
		if let Some(key) = series_key(path) {
			return Ok(Some(DeepLinkResult::Manga { key }));
		}
		if path.starts_with("/chapter/") {
			// A chapter path holds the series id, not the path its page lives at
			let response = fetch(&url)?;
			let html = response.get_html()?;
			let Some(series) = html.select("a[href*='/series/']").and_then(|mut links| {
				links.find_map(|link| {
					let href = link.attr("href")?;
					let path = href.strip_prefix_or_self(&params.base_url);
					series_key(path)
				})
			}) else {
				return Ok(None);
			};
			return Ok(Some(DeepLinkResult::Chapter {
				manga_key: series,
				key: path.into(),
			}));
		}
		Ok(None)
	}
}
