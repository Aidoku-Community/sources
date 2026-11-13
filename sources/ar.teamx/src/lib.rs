#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Manga, MangaPageResult, MangaStatus,
	Page, PageContent, Result, Source, Viewer,
	alloc::{String, Vec, format, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::net::{Request, TimeUnit, set_rate_limit},
	prelude::*,
};

mod helpers;
use helpers::{chapter_path, manga_path, parse_card, parse_chapter};

const BASE_URL: &str = "https://olympustaff.com";
const THUMBNAIL_SUFFIX: &str = "thumbnail_";

struct TeamX;

impl Source for TeamX {
	fn new() -> Self {
		set_rate_limit(10, 1, TimeUnit::Seconds);
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		if let Some(query) = query.as_deref().filter(|query| !query.is_empty()) {
			let mut params = QueryParameters::new();
			params.push("keyword", Some(query));
			let url = format!("{BASE_URL}/search?{params}");
			let html = Request::get(url)?.html()?;
			let entries = html
				.select("div.tx-grid a.tx-card")
				.map(|els| {
					els.filter_map(|el| {
						let href = el.attr("abs:href")?;
						Some(Manga {
							key: manga_path(&href)?,
							title: el
								.select_first("h3")
								.and_then(|el| el.text())
								.or_else(|| el.attr("title"))?,
							cover: el.select_first("img").and_then(|img| {
								img.attr("abs:src")
									.map(|src| src.replace(THUMBNAIL_SUFFIX, ""))
							}),
							url: Some(href),
							..Default::default()
						})
					})
					.collect()
				})
				.ok_or_else(|| error!("Failed to select search results"))?;

			return Ok(MangaPageResult {
				entries,
				has_next_page: false,
			});
		}

		let mut params = QueryParameters::new();
		if page > 1 {
			params.push("page", Some(page.to_string().as_str()));
		}
		for filter in filters {
			match filter {
				FilterValue::Select { id, value } if !value.is_empty() => {
					params.push(&id, Some(value.as_str()));
				}
				FilterValue::MultiSelect { id, included, .. } => {
					for value in included {
						params.push(&id, Some(value.as_str()));
					}
				}
				_ => {}
			}
		}

		let mut url = format!("{BASE_URL}/series");
		if !params.is_empty() {
			url.push('?');
			url.push_str(&params.to_string());
		}

		let html = Request::get(url)?.html()?;
		let entries = html
			.select("div.listupd div.bsx")
			.map(|els| els.filter_map(|el| parse_card(&el)).collect())
			.ok_or_else(|| error!("Failed to select popular entries"))?;
		let has_next_page = html.select_first("a[rel=next]").is_some();

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
		let url = format!("{BASE_URL}{}", manga.key);
		let html = Request::get(&url)?.html()?;

		if needs_details {
			let info: Vec<String> = html
				.select(".full-list-info small")
				.map(|els| els.filter_map(|el| el.text()).collect())
				.unwrap_or_default();
			let value_after = |label: &str| {
				info.iter()
					.position(|text| text.trim().trim_end_matches(':').trim() == label)
					.and_then(|index| info.get(index + 1))
					.map(|text| text.trim().to_string())
			};

			manga.title = html
				.select_first("div.author-info-title h1")
				.and_then(|el| el.text())
				.unwrap_or(manga.title);
			manga.description = html
				.select_first("div.review-content")
				.and_then(|el| el.text());
			manga.cover = html
				.select_first("div.text-right img")
				.and_then(|el| el.attr("abs:src"));
			manga.tags = html.select("div.review-author-info a").map(|els| {
				els.filter_map(|el| el.text().map(|text| text.trim().to_string()))
					.collect()
			});
			manga.status = value_after("الحالة")
				.map(|status| match status.as_str() {
					"مستمرة" | "قادم قريبًا" => MangaStatus::Ongoing,
					"مكتمل" => MangaStatus::Completed,
					"متوقف" => MangaStatus::Hiatus,
					_ => MangaStatus::Unknown,
				})
				.unwrap_or_default();
			manga.artists = value_after("الرسام")
				.filter(|artist| artist != "غير معروف")
				.map(|artist| vec![artist]);
			manga.viewer = match value_after("النوع") {
				Some(kind) if kind.contains("تون") => Viewer::Webtoon,
				_ => Viewer::Unknown,
			};
			manga.url = Some(url);
		}

		if needs_chapters {
			let mut chapters: Vec<Chapter> = html
				.select("div.chapter-card")
				.map(|els| els.filter_map(|el| parse_chapter(&el)).collect())
				.unwrap_or_default();

			let last_page = html
				.select("ul.pagination a.page-link")
				.map(|els| {
					els.filter_map(|el| el.text()?.trim().parse::<i32>().ok())
						.max()
						.unwrap_or(1)
				})
				.unwrap_or(1);

			if last_page > 1 {
				let key = &manga.key;
				let requests: Vec<Request> = (2..=last_page)
					.filter_map(|page| Request::get(format!("{BASE_URL}{key}?page={page}")).ok())
					.collect();
				for response in Request::send_all(requests) {
					let Ok(html) = response?.get_html() else {
						continue;
					};
					let mut page_chapters: Vec<Chapter> = html
						.select("div.chapter-card")
						.map(|els| els.filter_map(|el| parse_chapter(&el)).collect())
						.unwrap_or_default();
					chapters.append(&mut page_chapters);
				}
			}

			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let html = Request::get(format!("{BASE_URL}{}", chapter.key))?.html()?;
		html.select("div.image_list img, div.image_list canvas")
			.map(|els| {
				els.filter_map(|el| {
					let url = el
						.attr("abs:src")
						.filter(|url| !url.is_empty())
						.or_else(|| el.attr("abs:data-src"))?;
					Some(Page {
						content: PageContent::url(url),
						..Default::default()
					})
				})
				.collect()
			})
			.ok_or_else(|| error!("Failed to select page images"))
	}
}

impl DeepLinkHandler for TeamX {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		if let Some((manga_key, key)) = chapter_path(&url) {
			return Ok(Some(DeepLinkResult::Chapter { manga_key, key }));
		}
		Ok(manga_path(&url).map(|key| DeepLinkResult::Manga { key }))
	}
}

register_source!(TeamX, DeepLinkHandler);
