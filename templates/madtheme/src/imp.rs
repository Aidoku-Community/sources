use crate::{Params, helpers};
use aidoku::{
	Chapter, ContentRating, DeepLinkResult, FilterValue, Manga, MangaPageResult, MangaStatus, Page,
	PageContent, PageContext, Result,
	alloc::{String, Vec, string::ToString, vec},
	helpers::{element::ElementHelpers, string::StripPrefixOrSelf, uri::QueryParameters},
	imports::{
		html::{Document, Html},
		net::Request,
		std::{current_date, parse_date, send_partial_result},
	},
	prelude::*,
};

pub trait Impl {
	fn new() -> Self;

	fn params(&self) -> Params;

	fn get_search_manga_list(
		&self,
		params: &Params,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut qs = QueryParameters::new();
		qs.push("page", Some(&page.to_string()));
		qs.push("q", query.as_deref());
		qs.push("status", Some("all"));

		for filter in filters {
			match filter {
				FilterValue::Sort { id, index, .. } => {
					let value = match index {
						0 => "views",
						1 => "updated_at",
						2 => "created_at",
						3 => "name",
						4 => "rating",
						_ => "views",
					};
					qs.push(&id, Some(value));
				}
				FilterValue::Select { id, value } => {
					qs.set(&id, Some(&value));
				}
				FilterValue::MultiSelect { id, included, .. } => {
					for item in included {
						qs.push(&id, Some(&item));
					}
				}
				_ => {}
			}
		}

		let url = format!("{}/search?{qs}", params.base_url);
		let html = Request::get(url)?.html()?;

		Ok(MangaPageResult {
			entries: html
				.select(".book-detailed-item")
				.map(|els| {
					els.filter_map(|el| {
						let link = el.select_first("a")?;
						Some(Manga {
							key: link
								.attr("href")?
								.strip_prefix_or_self(&params.base_url)
								.into(),
							title: link.attr("title")?,
							cover: el.select_first("img")?.attr("abs:data-src"),
							..Default::default()
						})
					})
					.collect()
				})
				.unwrap_or_default(),
			has_next_page: html
				.select_first(".paginator > a.active + a:not([rel=next])")
				.is_some(),
		})
	}

	fn get_manga_update(
		&self,
		params: &Params,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let manga_url = format!("{}{}", params.base_url, manga.key);
		let html = Request::get(&manga_url)?.html()?;

		if needs_details {
			manga.title = html
				.select_first(".detail h1")
				.and_then(|h1| h1.text())
				.unwrap_or(manga.title);
			manga.cover = html
				.select_first("#cover img")
				.and_then(|img| img.attr("abs:data-src"))
				.or(manga.cover);
			manga.authors = html
				.select(".detail .meta > p > strong:contains(Authors) ~ a")
				.map(|els| {
					els.filter_map(|el| el.text())
						.map(|s| s.trim().trim_end_matches(',').trim().into())
						.collect()
				});
			manga.description = html
				.select_first(".summary .content, .summary .content ~ p")
				.and_then(|div| div.text());
			manga.url = Some(manga_url.clone());
			manga.tags = html
				.select(".detail .meta > p > strong:contains(Genres) ~ a")
				.map(|els| {
					els.filter_map(|el| el.text())
						.map(|s| s.trim().trim_end_matches(',').into())
						.collect()
				});
			manga.status = html
				.select_first(".detail .meta > p > strong:contains(Status) ~ a")
				.and_then(|el| el.text())
				.map(|text| match text.to_lowercase().as_str() {
					"ongoing" => MangaStatus::Ongoing,
					"completed" => MangaStatus::Completed,
					"on-hold" => MangaStatus::Hiatus,
					"canceled" => MangaStatus::Cancelled,
					_ => MangaStatus::Unknown,
				})
				.unwrap_or_default();
			let tags = manga.tags.as_deref().unwrap_or(&[]);
			manga.content_rating = if tags
				.iter()
				.any(|e| matches!(e.as_str(), "Adult" | "Hentai" | "Mature" | "Smut"))
			{
				ContentRating::NSFW
			} else if tags.iter().any(|e| e == "Ecchi") {
				ContentRating::Suggestive
			} else if params.default_rating != ContentRating::Unknown {
				params.default_rating
			} else {
				ContentRating::Safe
			};
			manga.viewer = params.default_viewer;

			send_partial_result(&manga);
		}

		if needs_chapters {
			fn parse_chapter_elements(html: &Document, params: &Params) -> Vec<Chapter> {
				html.select("#chapter-list > li")
					.map(|els| {
						els.filter_map(|el| {
							let a = el.select_first("a")?;
							let link = a.attr("abs:href")?;
							let title = el.select_first(".chapter-title")?.text()?;
							let chapter_number = helpers::find_first_f32(&title);
							Some(Chapter {
								key: link.strip_prefix_or_self(&params.base_url).into(),
								title: if title.as_str()
									!= format!("Chapter {}", chapter_number.unwrap_or(0.0))
								{
									Some(title)
								} else {
									None
								},
								chapter_number,
								date_uploaded: el
									.select_first(".chapter-update")
									.and_then(|el| el.text())
									.map(|s| {
										parse_date(s, &params.date_format).unwrap_or(current_date())
									}),
								url: Some(link),
								..Default::default()
							})
						})
						.collect()
					})
					.unwrap_or_default()
			}

			let fetch_api = html
				.select_first("div#show-more-chapters > span")
				.is_some_and(|el| el.attr("onclick").is_some_and(|s| s == "getChapters()"));

			let chapters = if fetch_api {
				let data = html
					.select_first("body > div.layout > script")
					.and_then(|el| el.data())
					.ok_or(error!("Cannot find script"))?;

				let url = format!(
					"{}/api/manga/{}/chapters/?source=detail",
					params.base_url,
					if params.use_slug_search {
						data.split_once("var bookSlug = \"")
							.ok_or(error!("String not found: `var bookSlug = \"`"))?
							.1
							.split_once("\";")
							.ok_or_else(|| error!("String not found: `\";`"))?
							.0
					} else {
						data.split_once("var bookId = ")
							.ok_or(error!("String not found: `var bookId = `"))?
							.1
							.split_once(";")
							.ok_or_else(|| error!("String not found: `;`"))?
							.0
					}
				);
				let html = Request::get(&url)?.html()?;
				parse_chapter_elements(&html, params)
			} else {
				parse_chapter_elements(&html, params)
			};

			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, params: &Params, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{}{}", params.base_url, chapter.key);
		let response = Request::get(&url)?.string()?;

		fn parse_pages(html: &Document) -> Vec<Page> {
			html.select("#chapter-images img, .chapter-image[data-src]")
				.map(|els| {
					els.filter_map(|el| {
						Some(Page {
							content: PageContent::url(el.attr("data-src")?),
							..Default::default()
						})
					})
					.collect()
				})
				.unwrap_or_default()
		}

		if response.contains("var chapImages = '") {
			Ok(response
				.split_once("var chapImages = '")
				.ok_or(error!("String not found: `var chapImages = '`"))?
				.1
				.split_once("';")
				.ok_or_else(|| error!("String not found: `';`"))?
				.0
				.split(',')
				.map(|s| s.to_string())
				.map(|url| Page {
					content: PageContent::url(url),
					..Default::default()
				})
				.collect())
		} else {
			let html = Html::parse_with_url(&response, url)?;
			let pages = parse_pages(&html);
			if pages.is_empty() {
				let text = html
					.select_first("#chapter__content > div.content-inner")
					.and_then(|el| el.text_with_newlines());
				if let Some(text) = text {
					Ok(vec![Page {
						content: PageContent::text(text),
						..Default::default()
					}])
				} else {
					// todo: the keiyoushi source has something like this
					// 	let chapter_id = html
					// 		.select_first("body > div.layout > script")
					// 		.and_then(|el| el.data())
					// 		.ok_or(error!("Cannot find script"))?
					// 		.split_once("var bookId = ")
					// 		.ok_or(error!("String not found: `var bookId = `"))?
					// 		.1
					// 		.split_once(";")
					// 		.ok_or_else(|| error!("String not found: `;`"))?
					// 		.0
					// 		.to_string();
					// 	let url = format!(
					// 		"{}/service/backend/chapterServer/?server_id=1&chapter_id={chapter_id}",
					// 		params.base_url
					// 	);
					// 	let html = Request::get(url)?
					// 		.header("Referer", &format!("{}/", params.base_url))
					// 		.html()?;
					// 	pages = parse_pages(&html);
					bail!("No content found")
				}
			} else {
				Ok(pages)
			}
		}
	}

	fn get_image_request(
		&self,
		params: &Params,
		url: String,
		_context: Option<PageContext>,
	) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", &format!("{}/", params.base_url)))
	}

	fn handle_deep_link(&self, params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(params.base_url.as_ref()) else {
			return Ok(None);
		};

		let slash_count = path.matches('/').count();

		if slash_count == 1 {
			// ex: https://toonily.me/solo-leveling
			Ok(Some(DeepLinkResult::Manga { key: path.into() }))
		} else {
			// ex: https://toonily.me/solo-leveling/chapter-1
			let second_slash = path[1..].find('/').ok_or(error!("Invalid URL"))? + 1;
			let manga_key = &path[..second_slash];
			Ok(Some(DeepLinkResult::Chapter {
				manga_key: manga_key.into(),
				key: path.into(),
			}))
		}
	}
}
