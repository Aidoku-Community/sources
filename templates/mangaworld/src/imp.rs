use super::Params;
use aidoku::{
	Chapter, ContentRating, DeepLinkResult, FilterValue, Manga, MangaPageResult, MangaStatus, Page,
	PageContent, PageContext, Result, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	helpers::{string::StripPrefixOrSelf, uri::QueryParameters},
	imports::{
		net::Request,
		std::{parse_date_with_options, send_partial_result},
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
		if query.is_some() {
			qs.push("keyword", query.as_deref());
		}

		for filter in filters {
			match filter {
				FilterValue::Sort { id, index, .. } => {
					let value = match index {
						0 => "most_read",
						1 => "less_read",
						2 => "newest",
						3 => "oldest",
						4 => "a-z",
						5 => "z-a",
						_ => "a-z",
					};
					qs.push(&id, Some(value));
				}
				FilterValue::Text { id, value } => {
					qs.set(&id, Some(&value));
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

		let url = format!("{}/archive?{qs}", params.base_url);
		let html = Request::get(url)?.html()?;

		let entries = html
			.select("div.comics-grid .entry")
			.map(|els| {
				els.filter_map(|el| {
					let link = el.select_first("a")?;
					Some(Manga {
						key: link
							.attr("href")?
							.strip_prefix_or_self(&params.base_url)
							.trim_end_matches("/")
							.into(),
						title: link.attr("title")?,
						cover: el.select_first("a.thumb img")?.attr("src"),
						..Default::default()
					})
				})
				.collect::<Vec<_>>()
			})
			.unwrap_or_default();
		let has_next_page = entries.len() == 16;

		Ok(MangaPageResult {
			entries,
			has_next_page,
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
			let info_element = html
				.select_first("div.comic-info")
				.ok_or(error!("Page not found"))?;

			manga.title = info_element
				.select_first("h1.name")
				.and_then(|h1| h1.text())
				.unwrap_or(manga.title);
			manga.cover = info_element
				.select_first(".thumb > img")
				.and_then(|img| img.attr("src"))
				.or(manga.cover);
			manga.artists = info_element
				.select_first("a[href*=/archive?artist=]")
				.and_then(|el| el.text())
				.map(|s| vec![s]);
			manga.authors = info_element
				.select_first("a[href*=/archive?author=]")
				.and_then(|el| el.text())
				.map(|s| vec![s]);
			manga.description = html.select_first("div#noidungm").and_then(|div| div.text());
			manga.url = Some(manga_url.clone());
			manga.tags = info_element
				.select("div.meta-data a.badge")
				.map(|els| els.filter_map(|el| el.text()).collect());
			manga.status = info_element
				.select_first("a[href*=/archive?status=]")
				.and_then(|el| el.text())
				.map(|text| match text.to_lowercase().as_str() {
					"in corso" => MangaStatus::Ongoing,
					"finito" => MangaStatus::Completed,
					"in pausa" => MangaStatus::Hiatus,
					"cancellato" => MangaStatus::Cancelled,
					_ => MangaStatus::Unknown,
				})
				.unwrap_or_default();
			let tags = manga.tags.as_deref().unwrap_or(&[]);
			manga.content_rating = if tags
				.iter()
				.any(|e| matches!(e.as_str(), "Adulti" | "Hentai" | "Maturo" | "Smut"))
			{
				ContentRating::NSFW
			} else if tags.iter().any(|e| e == "Ecchi") {
				ContentRating::Suggestive
			} else {
				ContentRating::Safe
			};
			manga.viewer = info_element
				.select_first("a[href*=/archive?type=]")
				.and_then(|el| el.text())
				.map(|text| match text.to_lowercase().as_str() {
					"manga" | "doujinshi" | "oneshot" => Viewer::RightToLeft,
					"manhua" | "manhwa" => Viewer::Webtoon,
					_ => Viewer::Unknown,
				})
				.unwrap_or_default();

			send_partial_result(&manga);
		}

		if needs_chapters {
			fn get_chapter_number(id: &str) -> Option<f32> {
				id.chars()
					.filter(|a| (*a >= '0' && *a <= '9') || *a == ' ' || *a == '.')
					.collect::<String>()
					.split(' ')
					.collect::<Vec<&str>>()
					.into_iter()
					.map(|a| a.parse::<f32>().unwrap_or(0.0))
					.find(|a| *a > 0.0)
			}

			manga.chapters = html.select(".chapters-wrapper .chapter").map(|els| {
				els.filter_map(|el| {
					let url = el.select_first("a.chap")?.attr("abs:href")?;
					let title = el.select_first("span.d-inline-block")?.text()?;
					let chapter_number = get_chapter_number(&title);
					Some(Chapter {
						key: url.strip_prefix_or_self(&params.base_url).into(),
						title: Some(title),
						chapter_number,
						date_uploaded: el
							.select(".chap-date")
							.and_then(|mut els| els.next_back())
							.and_then(|el| el.text())
							.and_then(|s| {
								parse_date_with_options(s, "dd MMMM yyyy", "it", "current")
							}),
						url: Some(url),
						..Default::default()
					})
				})
				.collect()
			});
		}

		Ok(manga)
	}

	fn get_page_list(&self, params: &Params, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{}{}?style=list", params.base_url, chapter.key);
		let html = Request::get(url)?.html()?;

		Ok(html
			.select("div#page img.page-image")
			.map(|els| {
				els.filter_map(|el| {
					Some(Page {
						content: PageContent::url(el.attr("src")?),
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default())
	}

	fn get_image_request(
		&self,
		params: &Params,
		url: String,
		_context: Option<PageContext>,
	) -> Result<Request> {
		// this doesn't look like it's necessary, but it's here for the possible future
		Ok(Request::get(url)?.header("Referer", &format!("{}/", params.base_url)))
	}

	fn handle_deep_link(&self, params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(params.base_url.as_ref()) else {
			return Ok(None);
		};

		const MANGA_PATH: &str = "/manga/";
		if !path.starts_with(MANGA_PATH) {
			return Ok(None);
		}

		let slash_count = path.matches('/').count();

		if slash_count <= 3 {
			// ex: https://www.mangaworld.cx/manga/2980/babylon-made-wa-nan-kounen
			Ok(Some(DeepLinkResult::Manga { key: path.into() }))
		} else {
			// ex: https://www.mangaworld.cx/manga/2980/babylon-made-wa-nan-kounen/read/64a7eab08c9c4e3f780c5932/1
			Ok(None)
		}
	}
}
