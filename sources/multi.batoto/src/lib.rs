#![no_std]
use aidoku::{
	BaseUrlProvider, Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue,
	ImageRequestProvider, Manga, MangaPageResult, MangaStatus, MigrationHandler, Page, PageContent,
	PageContext, Result, Source, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::{defaults::defaults_get, html::Document, net::Request, std::current_date},
	prelude::*,
};

mod helpers;
mod settings;

struct XBatCat;

impl Source for XBatCat {
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
		qs.push("page", Some(&page.to_string()));
		if query.is_some() {
			qs.push("word", query.as_deref());
		}
		qs.push("sort", Some("field_score"));
		for filter in filters {
			match filter {
				FilterValue::Sort { id, index, .. } => {
					qs.set(
						&id,
						Some(match index {
							0 => "field_score",
							1 => "field_follow",
							2 => "field_review",
							3 => "field_comment",
							4 => "field_chapter",
							5 => "field_upload",
							6 => "field_public",
							7 => "field_name",
							_ => "field_score",
						}),
					);
				}
				FilterValue::Select { id, value } => {
					qs.push(&id, Some(&value));
				}
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} => {
					let mut value = included.join(",");
					if !excluded.is_empty() {
						value.push_str(&format!("|{}", excluded.join(",")));
					}
					qs.push(&id, Some(&value));
				}
				_ => continue,
			}
		}
		let langs = settings::get_languages()?;
		if !langs.is_empty() {
			qs.push("lang", Some(&langs.join(",")));
		}
		let base_url = self.get_base_url()?;

		if base_url.starts_with("https://bato") {
			bail!("Change \"Base URL\" in settings.");
		}

		let url = format!("{base_url}/comics?{qs}");

		let html = Request::get(url)?.html()?;

		let entries = html
			.select("main > .grid > div")
			.map(|els| {
				els.filter_map(|el| {
					Some(Manga {
						key: helpers::get_manga_key(&el.select_first("a")?.attr("abs:href")?)?,
						title: el.select_first("img[title]")?.attr("title")?,
						cover: el.select_first("img").and_then(|el| el.attr("abs:src")),
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default();

		let has_next_page = html
			.select("main > .flex.items-center > .btn")
			.ok_or(error!("select_failed"))?
			.next_back()
			.is_none_or(|el| !el.has_class("btn-accent"));

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
		let base_url = self.get_base_url()?;
		let url = format!("{base_url}/title/{}", manga.key);
		let html = Request::get(&url)?.html()?;

		if needs_details {
			let details_selector = "main > div";
			let title_selector = "h3 > a";
			let cover_selector = "img";
			let artist_selector = ".space-y-2:not(.hidden) > div > a[href^=\"/artist\"]";
			let author_selector = ".space-y-2:not(.hidden) > div > a[href^=\"/author\"]";
			let tag_selector = "main > div .grid > .space-y-2:not(.hidden) > .flex > span > span.whitespace-nowrap";
			let status_selector = ".grid > .space-y-2:not(.hidden) > div > span:contains(original publication) + span";
			let bato_status_selector =
				".grid > .space-y-2:not(.hidden) > div > span:contains(upload status) + span";
			let viewer_selector =
				".grid > .space-y-2:not(.hidden) > div > span:contains(read direction) + span";

			let info_element = html
				.select_first(details_selector)
				.ok_or_else(|| error!("Missing details element"))?;
			manga.title = info_element
				.select_first(title_selector)
				.and_then(|el| el.text())
				.unwrap_or(manga.title);
			manga.cover = html
				.select_first(cover_selector)
				.and_then(|el| el.attr("abs:src"));
			manga.artists = html
				.select(artist_selector)
				.map(|els| els.filter_map(|el| el.text()).collect());
			manga.authors = html.select(author_selector).map(|els| {
				els.filter_map(|el| el.text())
					.map(|s| {
						s.trim_end_matches("(Story&Art)")
							.trim_end_matches("(Story)")
							.trim_end_matches("(Art)")
							.into()
					})
					.collect()
			});
			manga.description = html.select_first(".limit-html").and_then(|el| el.text());
			manga.url = Some(url);
			manga.tags = html
				.select(tag_selector)
				.map(|els| els.filter_map(|el| el.text()).collect());
			manga.status = html
				.select_first(status_selector)
				.or_else(|| html.select_first(bato_status_selector))
				.and_then(|el| el.text())
				.map(|s| {
					if s.contains("Ongoing") {
						MangaStatus::Ongoing
					} else if s.contains("Completed") {
						MangaStatus::Completed
					} else if s.contains("Hiatus") {
						MangaStatus::Hiatus
					} else if s.contains("Cancelled") {
						MangaStatus::Cancelled
					} else {
						MangaStatus::Unknown
					}
				})
				.unwrap_or_default();
			let tags = manga.tags.as_deref().unwrap_or_default();
			manga.content_rating = if html.select_first(".alert.alert-warning span b").is_some()
				|| tags
					.iter()
					.any(|e| matches!(e.as_str(), "Adult" | "Mature" | "Smut" | "Hentai"))
			{
				ContentRating::NSFW
			} else if tags.iter().any(|e| e == "Ecchi") {
				ContentRating::Suggestive
			} else {
				ContentRating::Safe
			};
			manga.viewer = if tags
				.iter()
				.any(|s| matches!(s.as_str(), "Manhwa" | "Webtoon"))
			{
				Viewer::Webtoon
			} else {
				html.select_first(viewer_selector)
					.and_then(|el| el.text())
					.map(|s| match s.as_str() {
						"Left to Right" => Viewer::LeftToRight,
						"Right to Left" => Viewer::RightToLeft,
						"Top to Bottom" => Viewer::Webtoon,
						_ => Viewer::Unknown,
					})
					.unwrap_or_default()
			};
		}

		if needs_chapters {
			let language_selector =
				"main > div .grid > .space-y-2:not(.hidden) > div.whitespace-nowrap > span + span";
			let chapter_selector =
				"div[data-name=\"chapter-list\"] > div.space-y-3 > div.border > div.flex > div";
			let link_selector = "a";
			let date_selector = "span[data-passed]";
			let scanlator_selector = "a > span.text-ellipsis";

			let language: Option<String> = html
				.select_first(language_selector)
				.and_then(|el| el.text())
				.map(|s| helpers::get_language_iso(&s).into());

			let parse_chapters = |html: &Document| -> Option<Vec<Chapter>> {
				html.select(chapter_selector).map(|els| {
					els.filter_map(|el| {
						let link = el.select_first(link_selector)?;
						let url = link.attr("abs:href")?;
						let key = helpers::get_chapter_key(&url)?;
						let info = helpers::parse_chapter_title(&link.text().unwrap_or_default());
						let now = current_date();
						let date_el = el.select_first(date_selector);
						let date_uploaded = date_el
							.as_ref()
							.and_then(|el| {
								if el.has_attr("data-passed") {
									el.attr("data-passed")
										.and_then(|data| data.parse::<i64>().ok())
										.map(|sec_ago| now - sec_ago)
								} else {
									None
								}
							})
							.or_else(|| {
								date_el.and_then(|el| el.text()).and_then(|s| {
									if s.ends_with("days ago") {
										s.split_whitespace()
											.next()
											.and_then(|s| s.parse::<i64>().ok())
											.map(|days_ago| now - days_ago * 24 * 60 * 60)
									} else {
										None
									}
								})
							})
							.or(Some(now));
						Some(Chapter {
							key,
							title: info.title,
							volume_number: info.volume,
							chapter_number: info.chapter,
							date_uploaded,
							scanlators: html
								.select_first(scanlator_selector)
								.and_then(|el| el.text())
								.map(|s| vec![s]),
							url: Some(url),
							language: language.clone(),
							..Default::default()
						})
					})
					.collect()
				})
			};

			let query = "query get_comic_chapterPager($comicId: ID!) {\n  \
					get_comic_chapterPager(comicId: $comicId){\n    \
						id data {\n      \
							order\n    \
						}\n  \
					}\n\
				}";
			let json = Request::post(format!("{base_url}/ap2/"))?
				.body(
					serde_json::json!({
						"query": query,
						"variables": {
							"comicId": manga.key
						}
					})
					.to_string(),
				)
				.header("Content-Type", "application/json")
				.json_owned::<serde_json::Value>()?;
			if let Some(objects) = json["data"]["get_comic_chapterPager"].as_array() {
				let urls = objects
					.iter()
					.step_by(2)
					.filter_map(|v| v["data"]["order"].as_i64())
					.map(|order| format!("{base_url}/title/{}?start={order}", manga.key))
					.collect::<Vec<_>>();
				if !urls.is_empty() {
					manga.chapters = Some(
						Request::send_all(urls.into_iter().map(|url| Request::get(url).unwrap()))
							.into_iter()
							.rev()
							.filter_map(|response| {
								let html = response.ok()?.get_html().ok()?;
								parse_chapters(&html)
							})
							.flatten()
							.collect::<Vec<_>>(),
					);
					return Ok(manga);
				}
			}

			// fallback: parse chapters from html
			manga.chapters = parse_chapters(&html);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let base_url = self.get_base_url()?;
		let url = format!("{base_url}/title/{}/{}", manga.key, chapter.key);
		let html = Request::get(url)?.html()?;

		let mut pages = Vec::new();

		for script in html
			.select("script[type=\"qwik/json\"]")
			.ok_or_else(|| error!("No script elements"))?
		{
			let script_text = script.data();
			let Some(script_text) = script_text else {
				continue;
			};
			for url in helpers::extract_image_urls(&script_text) {
				pages.push(Page {
					content: PageContent::url(url),
					..Default::default()
				});
			}
		}

		Ok(pages)
	}
}

impl DeepLinkHandler for XBatCat {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(url) = url
			.strip_prefix("http")
			.map(|url| url.trim_start_matches("s"))
			.and_then(|url| url.strip_prefix("://"))
		else {
			return Ok(None);
		};
		let Some(path) = url.find('/').and_then(|index| url.get(index + 1..)) else {
			return Ok(None);
		};

		const TITLE_PATH: &str = "title/";
		const SERIES_PATH: &str = "series/";

		if path.starts_with(TITLE_PATH) || path.starts_with(SERIES_PATH) {
			Ok(helpers::get_manga_key(path).map(|key| DeepLinkResult::Manga { key }))
		} else {
			Ok(None)
		}
	}
}

impl ImageRequestProvider for XBatCat {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url.replace("//k", "//n"))?)
	}
}

impl BaseUrlProvider for XBatCat {
	fn get_base_url(&self) -> Result<String> {
		Ok(defaults_get::<String>("url").unwrap_or_default())
	}
}

impl MigrationHandler for XBatCat {
	// example: 181119/a-familiar-feeling -> 181119
	fn handle_manga_migration(&self, key: String) -> Result<String> {
		if let Some(slash) = key.find('/') {
			Ok(key[..slash].into())
		} else {
			Ok(key)
		}
	}

	// chapter keys remain the same
	fn handle_chapter_migration(&self, _manga_key: String, chapter_key: String) -> Result<String> {
		Ok(chapter_key)
	}
}

register_source!(
	XBatCat,
	ImageRequestProvider,
	DeepLinkHandler,
	BaseUrlProvider,
	MigrationHandler
);
