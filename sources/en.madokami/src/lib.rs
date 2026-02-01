#![no_std]
use aidoku::{
	BasicLoginHandler, Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider,
	Manga, MangaPageResult, MangaStatus, Page, PageContent, PageContext, Result, Source,
	alloc::{string::String, vec::Vec},
	helpers::uri::{QueryParameters, decode_uri},
	imports::{
		net::Request,
		std::{current_date, parse_date_with_options, send_partial_result},
	},
	prelude::*,
};

mod auth;
mod helpers;
use auth::AuthedRequest;

const BASE_URL: &str = "https://manga.madokami.al";

struct Madokami;

impl Source for Madokami {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		_page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut query = query;

		for filter in filters {
			match filter {
				FilterValue::Text { id, value } => {
					if id == "author" {
						query = Some(value);
					}
				}
				FilterValue::Select { id, value } => {
					if id == "genre" {
						if let Some(category) = value.strip_prefix("Tag: ") {
							query = Some(format!("\"category:{category}\""));
						} else {
							query = Some(format!("\"genre:{value}\""));
						}
					}
				}
				_ => {}
			}
		}

		if query.is_none() {
			bail!("Enter a search term")
		}

		let mut qs = QueryParameters::new();
		qs.push("q", query.as_deref());
		let url = format!("{BASE_URL}/search?{qs}");
		let html = Request::get(url)?.authed()?.html()?;

		let entries = html
			.select("div.container table tbody tr td:nth-child(1) a:nth-child(1)")
			.map(|els| {
				els.filter_map(|el| {
					let url = el.attr("abs:href")?;
					let key = url.strip_prefix(BASE_URL)?.into();

					let path_segments: Vec<&str> = url.split('/').collect();
					let mut i = path_segments.len();
					let mut description = None;
					let mut title = None;

					// use last path component as path segment
					if let Some(last) = path_segments.last() {
						description = Some(decode_uri(last));
					}

					// use last path component that doesn't start with ! as title
					while i > 0 {
						i -= 1;
						let decoded = decode_uri(path_segments[i]);
						if !decoded.starts_with('!') {
							title = Some(decoded);
							break;
						}
					}
					let title = title?;

					Some(Manga {
						key,
						title,
						description,
						url: Some(url),
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default();

		Ok(MangaPageResult {
			entries,
			has_next_page: false,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{BASE_URL}{}", manga.key);
		let html = Request::get(url)?.authed()?.html()?;

		if needs_details {
			manga.title = html
				.select_first("div.manga-info span.title")
				.and_then(|el| el.text())
				.unwrap_or(manga.title);
			manga.cover = html
				.select_first("div.manga-info img[itemprop=\"image\"]")
				.and_then(|el| el.attr("src"));
			manga.authors = html
				.select("a[itemprop=\"author\"]")
				.map(|els| els.filter_map(|el| el.text()).collect());
			manga.tags = html
				.select("div.genres:not([itemprop]) a.tag")
				.and_then(|els| {
					Some(
						els.filter_map(|el| el.text())
							.chain(
								html.select("div.genres[itemprop] a.tag")?
									.filter_map(|el| el.text())
									.map(|s| format!("Tag: {s}")),
							)
							.collect(),
					)
				});
			manga.status = if html
				.select_first("span.scanstatus")
				.and_then(|el| el.text())
				.map(|s| s == "Yes")
				.unwrap_or_default()
			{
				MangaStatus::Completed
			} else {
				MangaStatus::Unknown
			};

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			manga.chapters = html
				.select("table#index-table > tbody > tr > td:nth-child(6) > a")
				.map(|els| {
					els.filter_map(|el| {
						let el = el.parent()?.parent()?;
						let href = el.select_first("td:nth-child(6) a")?.attr("href")?;
						let key = href[href.find("/reader")?..].into();
						let url = format!("{BASE_URL}{key}");
						Some(Chapter {
							key,
							title: el
								.select_first("td:nth-child(1) a")
								.and_then(|el| el.text()),
							date_uploaded: el
								.select_first("td:nth-child(3)")
								.and_then(|el| el.text())
								.and_then(|s| {
									if s.ends_with("ago") {
										Some(helpers::parse_relative_date(&s, current_date()))
									} else {
										parse_date_with_options(
											s,
											"yyyy-MM-dd HH:mm",
											"en_US",
											"current",
										)
									}
								}),
							url: Some(url),
							..Default::default()
						})
					})
					.rev()
					.collect()
				});
		}
		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{BASE_URL}{}", chapter.key);
		let html = Request::get(url)?.authed()?.html()?;

		let element = html
			.select_first("div#reader")
			.ok_or_else(|| error!("Missing reader element"))?;
		let path = element
			.attr("data-path")
			.ok_or_else(|| error!("Missing files path"))?;
		let files = element
			.attr("data-files")
			.and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok());

		files
			.map(|files| {
				files
					.into_iter()
					.map(|file| {
						let mut qs = QueryParameters::new();
						qs.push("path", Some(&path));
						qs.push("file", Some(&file));
						Page {
							content: PageContent::url(format!("{BASE_URL}/reader/image?{qs}")),
							..Default::default()
						}
					})
					.collect()
			})
			.ok_or_else(|| error!("Invalid page file list"))
	}
}

impl ImageRequestProvider for Madokami {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?
			.authed()?
			.header("Referer", BASE_URL)
			.header(
				"Accept",
				"image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8",
			)
			.header("Accept-Encoding", "gzip, deflate, br"))
	}
}

impl BasicLoginHandler for Madokami {
	fn handle_basic_login(&self, _key: String, username: String, password: String) -> Result<bool> {
		let response = Request::get(BASE_URL)?
			.header("Authorization", &auth::header(&username, &password))
			.send()?;
		Ok(response.status_code() == 200)
	}
}

impl DeepLinkHandler for Madokami {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		// https://manga.madokami.al/Manga/F/FI/FIGH/Fight%21%21%20Ippo/%21K-MANGA

		let Some(path) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};

		Ok(Some(DeepLinkResult::Manga { key: path.into() }))
	}
}

register_source!(Madokami, BasicLoginHandler, DeepLinkHandler);
