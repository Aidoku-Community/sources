#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Manga, MangaPageResult, MangaStatus,
	Page, PageContent, Result, Source, Viewer,
	alloc::{String, Vec, format, string::ToString, vec},
	helpers::uri::encode_uri_component,
	imports::{
		html::Element,
		net::{Request, TimeUnit, set_rate_limit},
	},
	prelude::*,
};

const BASE_URL: &str = "https://olympustaff.com";
const THUMBNAIL_SUFFIX: &str = "thumbnail_";

struct TeamX;

/// Turns any site URL into the manga key (`/series/<slug>`).
fn manga_path(url: &str) -> Option<String> {
	let path = url.strip_prefix(BASE_URL)?;
	let mut segments = path.trim_start_matches('/').split('/');
	if segments.next()? != "series" {
		return None;
	}
	let slug = segments.next().filter(|slug| !slug.is_empty())?;
	Some(format!("/series/{slug}"))
}

/// Turns a chapter URL into its manga key and chapter key.
fn chapter_path(url: &str) -> Option<(String, String)> {
	let path = url.strip_prefix(BASE_URL)?;
	let mut segments = path.trim_start_matches('/').split('/');
	if segments.next()? != "series" {
		return None;
	}
	let slug = segments.next().filter(|slug| !slug.is_empty())?;
	let chapter = segments.next().filter(|chapter| !chapter.is_empty())?;
	Some((
		format!("/series/{slug}"),
		format!("/series/{slug}/{chapter}"),
	))
}

fn parse_card(el: &Element) -> Option<Manga> {
	let link = el.select_first("a")?;
	let href = link.attr("abs:href")?;
	let img = el.select_first("img");
	let cover = img
		.as_ref()
		.and_then(|img| img.attr("abs:data-src").filter(|src| !src.is_empty()))
		.or_else(|| img.and_then(|img| img.attr("abs:src")));
	Some(Manga {
		key: manga_path(&href)?,
		title: link.attr("title")?,
		cover,
		url: Some(href),
		..Default::default()
	})
}

fn parse_chapter(el: &Element) -> Option<Chapter> {
	if el.select_first("span.locked").is_some() {
		return None;
	}
	let link = el.select_first("a")?;
	let href = link.attr("abs:href")?;
	let number = el.attr("data-number").and_then(|value| value.parse().ok());
	let extra_title = el
		.select_first("div.chapter-info div.chapter-title")
		.and_then(|el| el.text())
		.filter(|title| !title.trim().is_empty());

	let mut title = match number {
		Some(number) => format!("الفصل {number}"),
		None => String::new(),
	};
	if let Some(extra_title) = extra_title {
		let number = el.attr("data-number").unwrap_or_default();
		let generic = [
			number.clone(),
			format!("الفصل {number}"),
			format!("الفصل رقم {number}"),
		];
		if !generic.iter().any(|generic| generic == extra_title.trim()) {
			title.push_str(" - ");
			title.push_str(extra_title.trim());
		}
	}
	title.push('\u{200F}');

	Some(Chapter {
		key: href.strip_prefix(BASE_URL)?.to_string(),
		title: Some(title),
		chapter_number: number,
		date_uploaded: el
			.attr("data-date")
			.and_then(|value| value.parse::<i64>().ok())
			.map(|seconds| seconds * 1000),
		url: Some(href),
		..Default::default()
	})
}

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
		let query = query.unwrap_or_default();
		let query = query.trim();

		if !query.is_empty() {
			let url = format!("{BASE_URL}/search?keyword={}", encode_uri_component(query));
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

		let mut url = format!("{BASE_URL}/series");
		if page > 1 {
			url.push_str(&format!("?page={page}"));
		}
		for filter in filters {
			match filter {
				FilterValue::Select { id, value } if !value.is_empty() => {
					url.push(if url.contains('?') { '&' } else { '?' });
					url.push_str(&format!("{id}={}", encode_uri_component(&value)));
				}
				FilterValue::MultiSelect { id, included, .. } => {
					for value in included {
						url.push(if url.contains('?') { '&' } else { '?' });
						url.push_str(&format!("{id}={}", encode_uri_component(&value)));
					}
				}
				_ => {}
			}
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
