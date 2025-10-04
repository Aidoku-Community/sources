#![no_std]
use aidoku::{
	alloc::{string::ToString, vec, String, Vec},
	helpers::uri::{encode_uri_component, QueryParameters},
	imports::{
		error::AidokuError,
		html::{Document, Html},
		net::Request,
		std::{current_date, parse_date, send_partial_result},
	},
	prelude::*,
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider,
	Manga, MangaPageResult, MangaStatus, Page, PageContent, Result, Source, Viewer,
};

mod home;
mod models;
mod settings;

use models::*;

const BASE_URL: &str = "https://mangafire.to";

struct MangaFire;

impl Source for MangaFire {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut qs = QueryParameters::new();

		let mut author = None;

		// parse filters
		for filter in filters {
			match filter {
				FilterValue::Text { id, value } => match id.as_str() {
					"author" => {
						author = Some(encode_uri_component(value.to_lowercase().replace(' ', "-")));
					}
					_ => return Err(AidokuError::Message("Invalid text filter id".into())),
				},
				FilterValue::Sort { index, .. } => {
					let value = match index {
						0 => "most_relevance",
						1 => "recently_updated",
						2 => "recently_added",
						3 => "release_date",
						4 => "trending",
						5 => "title_az",
						6 => "scores",
						7 => "mal_scores",
						8 => "most_viewed",
						9 => "most_favourited",
						_ => return Err(AidokuError::Message("Invalid sort filter index".into())),
					};
					qs.push("sort", Some(value));
				}
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} => {
					for option in included {
						qs.push(&id, Some(&option));
					}
					for option in excluded {
						qs.push(&id, Some(&format!("-{option}")));
					}
				}
				FilterValue::Select { id, value } => {
					qs.push(&id, Some(&value));
				}
				_ => {}
			}
		}

		if let Some(query) = query {
			qs.push("keyword", Some(&query));
		}

		let url = if let Some(author) = author {
			format!(
				"{BASE_URL}/author/{author}\
					?page={page}\
					&{qs}"
			)
		} else {
			format!(
				"{BASE_URL}/filter\
					?page={page}\
					&{qs}",
			)
		};

		let mut entries = Vec::new();
		let mut has_next_page = false;

		let langs = settings::get_languages()?;
		for lang in langs {
			let html = Request::get(format!("{url}&language%5B%5D={lang}"))?
				.header("Referer", &format!("{BASE_URL}/"))
				.html()?;
			let result = parse_manga_page(&html);
			entries.extend(result.entries);
			has_next_page = result.has_next_page || has_next_page;
		}

		// remove duplicates
		let mut seen = Vec::new();
		entries.retain(|item| {
			if seen.contains(&item.key) {
				false
			} else {
				seen.push(item.key.clone());
				true
			}
		});

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let manga_url = format!("{BASE_URL}{}", manga.key);

		if needs_details {
			let html = Request::get(&manga_url)?.html()?;

			let content = html
				.select_first(".main-inner:not(.manga-bottom)")
				.ok_or(error!("manga details element missing"))?;
			let meta = html
				.select_first(".meta")
				.ok_or(error!("metadata element missing"))?;

			manga.title = content
				.select_first("h1")
				.and_then(|e| e.text())
				.unwrap_or_default();
			manga.cover = content
				.select_first(".poster img")
				.and_then(|e| e.attr("src"));
			manga.authors = meta
				.select_first("span:contains(Author:) + span")
				.and_then(|e| e.text())
				.map(|txt| vec![txt]);
			manga.description = html
				.select_first("#synopsis .modal-content")
				.and_then(|e| e.text());
			manga.url = Some(manga_url.clone());
			manga.tags = meta
				.select_first("span:contains(Genres:) + span")
				.and_then(|e| e.text())
				.map(|txt| txt.split(',').map(|s| s.trim().to_string()).collect());
			manga.status = content
				.select_first(".info > p")
				.and_then(|e| e.text())
				.map(|txt| match txt.to_lowercase().as_str() {
					"releasing" => MangaStatus::Ongoing,
					"completed" => MangaStatus::Completed,
					"on_hiatus" => MangaStatus::Hiatus,
					"discontinued" => MangaStatus::Cancelled,
					_ => MangaStatus::Unknown,
				})
				.unwrap_or_default();
			manga.content_rating = manga
				.tags
				.as_ref()
				.map(|tags| {
					if tags.iter().any(|tag| tag == "Ecchi") {
						ContentRating::Suggestive
					} else {
						ContentRating::Unknown
					}
				})
				.unwrap_or_default();
			manga.viewer = content
				.select_first(".info > .min-info > a")
				.and_then(|e| e.text())
				.map(|txt| match txt.as_str() {
					"Manhua" => Viewer::Webtoon,
					"Manhwa" => Viewer::Webtoon,
					"Manga" => Viewer::RightToLeft,
					_ => Viewer::RightToLeft,
				})
				.unwrap_or(Viewer::RightToLeft);

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let manga_id = manga_url
				.rsplit('.')
				.next()
				.map(|s| s.trim().to_string())
				.ok_or(error!("missing manga id"))?;

			let mut chapters = Vec::new();

			let languages = settings::get_languages()?;
			for lang in &languages {
				let ajax_manga_url = format!("{BASE_URL}/ajax/manga/{manga_id}/chapter/{lang}");
				let manga_list = Request::get(&ajax_manga_url)?
					.send()?
					.get_json::<AjaxResponse<String>>()
					.map(|response| Html::parse_fragment(&response.result))??
					.select("li")
					.ok_or(error!("failed manga_list select"))?;

				let ajax_read_url = format!("{BASE_URL}/ajax/read/{manga_id}/chapter/{lang}");
				let read_list = Request::get(&ajax_read_url)?
					.send()?
					.get_json::<AjaxResponse<AjaxRead>>()
					.map(|response| {
						Html::parse_fragment_with_url(&response.result.html, BASE_URL)
					})??
					.select("ul a")
					.ok_or(error!("failed read_list select"))?;

				chapters.extend(manga_list.zip(read_list).filter_map(|(m, r)| {
					let link = r.select_first("a")?;
					let key = format!("chapter/{}", r.attr("data-id")?);

					let url = link.attr("abs:href")?;

					let number = m.attr("data-number")?;
					if number != r.attr("data-number")? {
						return None;
					}

					let title = link.text().map(|title| {
						let prefix = format!("Chap {number}:");
						if title.starts_with(&prefix) {
							title[prefix.len()..].trim().to_string()
						} else {
							title
						}
					});
					let date_uploaded = m
						.select("span")
						.and_then(|els| els.get(1))
						.and_then(|el| el.text())
						.and_then(|txt| parse_date(txt, "MMM dd, yyyy"))
						.unwrap_or_else(current_date); // fallback for relative dates

					Some(Chapter {
						key,
						title,
						chapter_number: number.parse::<f32>().ok(),
						date_uploaded: Some(date_uploaded),
						url: Some(url),
						language: Some(lang.clone()),
						..Default::default()
					})
				}));
			}
			if languages.len() > 1 {
				chapters.sort_by_key(|c| core::cmp::Reverse(c.chapter_number.map(|n| n as i32)));
			}

			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let ajax_url = format!("{BASE_URL}/ajax/read/{}", chapter.key);

		Request::get(&ajax_url)?
			.send()?
			.get_json::<AjaxResponse<AjaxPageList>>()
			.map(|response| {
				response
					.result
					.images
					.iter()
					.filter_map(|img| {
						let url = img.first()?.as_str()?;
						Some(Page {
							content: PageContent::url(url),
							..Default::default()
						})
					})
					.collect()
			})
	}
}

impl ListingProvider for MangaFire {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let url = match listing.id.as_str() {
			"Newest" => format!("{BASE_URL}/newest?page={page}"),
			"Updated" => format!("{BASE_URL}/updated?page={page}"),
			"Added" => format!("{BASE_URL}/added?page={page}"),
			_ => bail!("Invalid listing ID"),
		};
		let html = Request::get(url)?
			.header("Referer", &format!("{BASE_URL}/"))
			.html()?;
		Ok(parse_manga_page(&html))
	}
}

impl DeepLinkHandler for MangaFire {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};

		const MANGA_PATH: &str = "/manga";
		const CHAPTER_PATH: &str = "/read";

		if path.starts_with(MANGA_PATH) {
			// ex: https://mangafire.to/manga/one-piecee.dkw
			Ok(Some(DeepLinkResult::Manga { key: path.into() }))
		} else if let Some(remaining_path) = path.strip_prefix(CHAPTER_PATH) {
			// ex: https://mangafire.to/read/one-piecee.dkw/en/chapter-1
			let end = remaining_path
				.find('/')
				// get the second slash
				.and_then(|i| remaining_path[i + 1..].find('/').map(|j| i + 1 + j))
				.unwrap_or(remaining_path.len());
			let key = format!("{MANGA_PATH}{}", &remaining_path[..end]);
			// can't get chapter key due to missing data-id
			Ok(Some(DeepLinkResult::Manga { key }))
		} else {
			Ok(None)
		}
	}
}

fn parse_manga_page(html: &Document) -> MangaPageResult {
	MangaPageResult {
		entries: html
			.select(".original.card-lg .unit .inner")
			.map(|els| {
				els.filter_map(|element| {
					let title_element = element.select_first(".info > a")?;
					let title = title_element.own_text().unwrap_or_default();
					let url = title_element.attr("abs:href")?;
					let key = url.strip_prefix(BASE_URL).map(String::from)?;
					let cover = element.select_first("img")?.attr("abs:src");
					Some(Manga {
						key,
						title,
						cover,
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default(),
		has_next_page: html
			.select_first(".page-item.active + .page-item .page-link")
			.is_some(),
	}
}

register_source!(MangaFire, Home, ListingProvider, DeepLinkHandler);
