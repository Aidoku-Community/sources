#![no_std]
use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent,
	HomeLayout, ImageRequestProvider, Listing, ListingProvider, Manga, MangaPageResult,
	MangaStatus, Page, PageContent, Result, Source, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::{net::Request, std::send_partial_result},
	prelude::*,
};

const BASE_URL: &str = "https://mangapill.com";

struct MangaPill;

fn parse_status(text: &str) -> MangaStatus {
	let lower = text.to_ascii_lowercase();
	if lower.contains("publishing") {
		MangaStatus::Ongoing
	} else if lower.contains("finished") {
		MangaStatus::Completed
	} else if lower.contains("on hiatus") {
		MangaStatus::Hiatus
	} else if lower.contains("discontinued") {
		MangaStatus::Cancelled
	} else {
		MangaStatus::Unknown
	}
}

fn parse_viewer(tags: &[String]) -> Viewer {
	if tags.iter().any(|t| t == "Manhwa" || t == "Manhua") {
		Viewer::Webtoon
	} else {
		Viewer::RightToLeft
	}
}

fn parse_content_rating(tags: &[String]) -> ContentRating {
	if tags.iter().any(|t| t == "Hentai") {
		ContentRating::NSFW
	} else if tags.iter().any(|t| t == "Ecchi") {
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	}
}

impl Source for MangaPill {
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
		let page_str = page.to_string();
		qs.push("page", Some(&page_str));

		if let Some(ref q) = query {
			qs.push("q", Some(q.as_str()));
		} else {
			for filter in filters {
				match filter {
					FilterValue::Select { id, value } => {
						qs.push(&id, Some(&value));
					}
					FilterValue::MultiSelect { id, included, .. } => {
						for v in &included {
							qs.push(&id, Some(v.as_str()));
						}
					}
					_ => {}
				}
			}
		}

		let url = format!("{BASE_URL}/search?{qs}");
		let html = Request::get(url)?.html()?;

		let entries = html
			.select(".grid > div:not([class])")
			.map(|els| {
				els.filter_map(|el| {
					let key = el.select_first("a")?.attr("href")?;
					let title = el
						.select_first("div[class] > a")
						.and_then(|e| e.text())
						.unwrap_or_default();
					let cover = el.select_first("img").and_then(|img| img.attr("data-src"));
					Some(Manga {
						key,
						title,
						cover,
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default();

		Ok(MangaPageResult {
			entries,
			has_next_page: html.select_first("a.btn.btn-sm").is_some(),
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{BASE_URL}{}", manga.key);
		let html = Request::get(url)?.html()?;

		if needs_details {
			manga.cover = html
				.select_first("div.container > div:first-child > div:first-child > img")
				.and_then(|img| img.attr("data-src"));

			manga.description = html
				.select_first(
					"div.container > div:first-child > div:last-child > div:nth-child(2) > p",
				)
				.and_then(|el| el.text());

			let status_text = html
				.select_first(
					"div.container > div:first-child > div:last-child > div:nth-child(3) > div:nth-child(2) > div",
				)
				.and_then(|el| el.text())
				.unwrap_or_default();
			manga.status = parse_status(&status_text);

			let tags: Vec<String> = html
				.select("a[href*=genre]")
				.map(|els| els.filter_map(|el| el.text()).collect())
				.unwrap_or_default();

			manga.viewer = parse_viewer(&tags);
			manga.content_rating = parse_content_rating(&tags);

			if !tags.is_empty() {
				manga.tags = Some(tags);
			}

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			manga.chapters = html.select("#chapters > div > a").map(|els| {
				els.filter_map(|el| {
					let key = el.attr("href")?;
					let url = format!("{BASE_URL}{key}");
					Some(Chapter {
						key,
						title: el.text(),
						url: Some(url),
						..Default::default()
					})
				})
				.collect()
			});
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{BASE_URL}{}", chapter.key);
		let html = Request::get(url)?.html()?;

		Ok(html
			.select("chapter-page img")
			.map(|els| {
				els.filter_map(|img| {
					let url = img.attr("data-src")?;
					Some(Page {
						content: PageContent::url(url),
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default())
	}
}

impl ListingProvider for MangaPill {
	fn get_manga_list(&self, _listing: Listing, _page: i32) -> Result<MangaPageResult> {
		let html = Request::get(format!("{BASE_URL}/chapters"))?.html()?;

		let entries = html
			.select(".grid > div:not([class])")
			.map(|els| {
				els.filter_map(|el| {
					let key = el.select_first("a[href^='/manga/']")?.attr("href")?;
					let title = el
						.select_first("a:not(:first-child) > div")
						.and_then(|e| e.text())
						.unwrap_or_default();
					let cover = el.select_first("img").and_then(|img| img.attr("data-src"));
					Some(Manga {
						key,
						title,
						cover,
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
}

impl Home for MangaPill {
	fn get_home(&self) -> Result<HomeLayout> {
		let html = Request::get(format!("{BASE_URL}/chapters"))?.html()?;

		let entries: Vec<Manga> = html
			.select(".grid > div:not([class])")
			.map(|els| {
				els.filter_map(|el| {
					let key = el.select_first("a[href^='/manga/']")?.attr("href")?;
					let title = el
						.select_first("a:not(:first-child) > div")
						.and_then(|e| e.text())
						.unwrap_or_default();
					let cover = el.select_first("img").and_then(|img| img.attr("data-src"));
					Some(Manga {
						key,
						title,
						cover,
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default();

		Ok(HomeLayout {
			components: vec![HomeComponent {
				title: Some("Latest Updates".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::Scroller {
					entries: entries.into_iter().map(Into::into).collect(),
					listing: None,
				},
			}],
		})
	}
}

impl DeepLinkHandler for MangaPill {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};

		if path.starts_with("/manga/") {
			Ok(Some(DeepLinkResult::Manga { key: path.into() }))
		} else {
			Ok(None)
		}
	}
}

impl ImageRequestProvider for MangaPill {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", &format!("{BASE_URL}/")))
	}
}

register_source!(
	MangaPill,
	ListingProvider,
	Home,
	DeepLinkHandler,
	ImageRequestProvider
);

#[cfg(test)]
mod test {
	use crate::{MangaPill, parse_content_rating, parse_status, parse_viewer};
	use aidoku::{
		ContentRating, DeepLinkHandler, DeepLinkResult, MangaStatus, Viewer,
		alloc::{String, Vec, vec},
	};
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn test_parse_status() {
		assert!(matches!(parse_status("Publishing"), MangaStatus::Ongoing));
		assert!(matches!(parse_status("Finished"), MangaStatus::Completed));
		assert!(matches!(parse_status("On Hiatus"), MangaStatus::Hiatus));
		assert!(matches!(
			parse_status("Discontinued"),
			MangaStatus::Cancelled
		));
		assert!(matches!(
			parse_status("Unknown Status"),
			MangaStatus::Unknown
		));
		assert!(matches!(parse_status(""), MangaStatus::Unknown));
	}

	#[aidoku_test]
	fn test_parse_viewer() {
		let manhwa: Vec<String> = vec!["Manhwa".into()];
		let manhua: Vec<String> = vec!["Manhua".into()];
		let manga: Vec<String> = vec!["Action".into(), "Fantasy".into()];
		let empty: Vec<String> = vec![];

		assert!(matches!(parse_viewer(&manhwa), Viewer::Webtoon));
		assert!(matches!(parse_viewer(&manhua), Viewer::Webtoon));
		assert!(matches!(parse_viewer(&manga), Viewer::RightToLeft));
		assert!(matches!(parse_viewer(&empty), Viewer::RightToLeft));
	}

	#[aidoku_test]
	fn test_parse_content_rating() {
		let hentai: Vec<String> = vec!["Hentai".into()];
		let ecchi: Vec<String> = vec!["Ecchi".into()];
		let safe: Vec<String> = vec!["Action".into()];
		let empty: Vec<String> = vec![];

		assert!(matches!(parse_content_rating(&hentai), ContentRating::NSFW));
		assert!(matches!(
			parse_content_rating(&ecchi),
			ContentRating::Suggestive
		));
		assert!(matches!(parse_content_rating(&safe), ContentRating::Safe));
		assert!(matches!(parse_content_rating(&empty), ContentRating::Safe));
	}

	#[aidoku_test]
	fn test_deep_link_manga() {
		let source = MangaPill;
		let result = source
			.handle_deep_link("https://mangapill.com/manga/123-title".into())
			.expect("deep link failed");
		assert_eq!(
			result,
			Some(DeepLinkResult::Manga {
				key: "/manga/123-title".into()
			})
		);
	}

	#[aidoku_test]
	fn test_deep_link_chapters_page() {
		let source = MangaPill;
		let result = source
			.handle_deep_link("https://mangapill.com/chapters".into())
			.expect("deep link failed");
		assert_eq!(result, None);
	}

	#[aidoku_test]
	fn test_deep_link_wrong_domain() {
		let source = MangaPill;
		let result = source
			.handle_deep_link("https://example.com/manga/123".into())
			.expect("deep link failed");
		assert_eq!(result, None);
	}
}
