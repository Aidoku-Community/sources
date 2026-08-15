#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, DynamicFilters, Filter, FilterValue,
	ImageRequestProvider, Listing, ListingProvider, Manga, MangaPageResult, MangaStatus, Page,
	PageContent, PageContext, Result, SelectFilter, Source, Viewer,
	alloc::{String, Vec, borrow::Cow, string::ToString, vec},
	helpers::{
		string::StripPrefixOrSelf,
		uri::{QueryParameters, encode_uri_component},
	},
	imports::{html::Html, net::Request, std::send_partial_result},
	prelude::*,
};

mod helpers;
mod models;
use helpers::*;
use models::*;

const BASE_URL: &str = "https://rawdevart.art";
// the site ignores these two letters when routing
const ID_PREFIX: &str = "ne";
const DATE_FORMAT: &str = "yyyy-MM-dd'T'HH:mm:ss.SSSXXX";

// same order as the options in res/filters.json
const SORTS: &[&str] = &["", "most_viewed", "most_viewed_today"];

pub fn manga_url(key: &str) -> String {
	format!("{BASE_URL}/g/{ID_PREFIX}{key}")
}

pub fn chapter_url(manga_key: &str, key: &str) -> String {
	format!("{BASE_URL}/read/{ID_PREFIX}{manga_key}/chapter-{key}")
}

struct Rawdevart;

impl Source for Rawdevart {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		// the search endpoint takes none of the other filters, so a query overrides them
		if let Some(query) = query {
			return Self::parse_manga_list(&format!(
				"{BASE_URL}/spa/search?query={}&page={page}",
				encode_uri_component(query)
			));
		}

		let mut genre = String::from("all");
		let mut params = QueryParameters::new();

		for filter in filters {
			match filter {
				FilterValue::Select { id, value } => {
					if value.is_empty() {
						continue;
					}
					match id.as_str() {
						"genre" => genre = value,
						"status" => params.push("status", Some(&value)),
						_ => continue,
					}
				}
				FilterValue::Sort { index, .. } => {
					let Some(sort) = SORTS.get(index as usize).filter(|sort| !sort.is_empty())
					else {
						continue;
					};
					params.push("sort", Some(sort));
				}
				_ => continue,
			}
		}
		params.push("page", Some(&page.to_string()));

		Self::parse_manga_list(&format!("{BASE_URL}/spa/genre/{genre}?{params}"))
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let MangaDetailsResponse {
			detail,
			tags,
			authors,
			chapters,
		} = Request::get(format!("{BASE_URL}/spa/manga/{}", manga.key))?
			.json_owned::<MangaDetailsResponse>()?;

		if needs_details && let Some(detail) = detail {
			if let Some(title) = detail.manga_name {
				manga.title = String::from(title.trim());
			}
			if let Some(cover) = detail.manga_cover_img_full.or(detail.manga_cover_img) {
				manga.cover = Some(cover);
			}
			manga.url = Some(manga_url(&manga.key));
			manga.description = detail
				.manga_description
				.as_deref()
				.map(strip_html)
				.filter(|description| !description.is_empty());

			let authors = authors
				.unwrap_or_default()
				.into_iter()
				.filter_map(|author| author.author_name)
				.map(|author| String::from(author.trim()))
				.filter(|author| !author.is_empty())
				.collect::<Vec<String>>();
			manga.authors = (!authors.is_empty()).then_some(authors);

			let tags = tags
				.unwrap_or_default()
				.into_iter()
				.filter_map(|tag| tag.tag_name)
				.map(|tag| String::from(tag.trim()))
				.filter(|tag| !tag.is_empty())
				.collect::<Vec<String>>();
			manga.content_rating = content_rating(&tags);
			manga.tags = (!tags.is_empty()).then_some(tags);

			manga.status = match detail.manga_status {
				Some(true) => MangaStatus::Completed,
				Some(false) => MangaStatus::Ongoing,
				None => MangaStatus::Unknown,
			};
			// the "manhwa" and "manhua" genres exist but hold no entries, so everything is japanese
			manga.viewer = Viewer::RightToLeft;

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			manga.chapters = Some(
				chapters
					.unwrap_or_default()
					.into_iter()
					.filter_map(|chapter| chapter.into_chapter(&manga.key))
					.collect(),
			);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{BASE_URL}/spa/manga/{}/{}", manga.key, chapter.key);
		let response = Request::get(url)?.json_owned::<ChapterPagesResponse>()?;

		// the endpoint answers with a null detail for chapters it doesn't know, which is a
		// failure rather than an empty chapter
		let Some(detail) = response.chapter_detail else {
			bail!("chapter {} of manga {} not found", chapter.key, manga.key);
		};

		// page images are given as paths relative to the image server
		let server = detail
			.server
			.filter(|server| !server.is_empty())
			.or_else(|| detail.slaves.unwrap_or_default().into_iter().next())
			.unwrap_or_else(|| format!("{BASE_URL}/"));

		let html = Html::parse_fragment(detail.chapter_content.unwrap_or_default())?;

		let pages = html
			.select(".chapter-img img")
			.map(|elements| {
				elements
					.filter_map(|element| {
						// resolved by hand rather than through "abs:", which needs the document to
						// carry a base url that a fragment doesn't reliably keep
						let src = element
							.attr("data-src")
							.or_else(|| element.attr("src"))
							.filter(|src| !src.is_empty())?;
						let url = if src.starts_with("http") {
							src
						} else {
							format!(
								"{}/{}",
								server.trim_end_matches('/'),
								src.trim_start_matches('/')
							)
						};
						Some(Page {
							content: PageContent::url(url),
							..Default::default()
						})
					})
					.collect::<Vec<Page>>()
			})
			.unwrap_or_default();

		Ok(pages)
	}
}

impl Rawdevart {
	fn parse_manga_list(url: &str) -> Result<MangaPageResult> {
		let response = Request::get(url)?.json_owned::<MangaListResponse>()?;

		let has_next_page = response
			.pagi
			.and_then(|pagi| pagi.button)
			.is_some_and(|button| button.next != 0);

		Ok(MangaPageResult {
			entries: response.manga_list.into_iter().map(Manga::from).collect(),
			has_next_page,
		})
	}
}

impl ListingProvider for Rawdevart {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let url = match listing.id.as_str() {
			"popular" => format!("{BASE_URL}/spa/genre/all?page={page}&sort=most_viewed"),
			"trending" => format!("{BASE_URL}/spa/genre/all?page={page}&sort=most_viewed_today"),
			_ => format!("{BASE_URL}/spa/latest-manga?page={page}"),
		};
		Self::parse_manga_list(&url)
	}
}

impl DynamicFilters for Rawdevart {
	// the genre list is fetched instead of hardcoded, so new genres are picked up automatically
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		let response =
			Request::get(format!("{BASE_URL}/spa/genre/all"))?.json_owned::<MangaListResponse>()?;
		let Some(genre_opt) = response.genre_opt else {
			return Ok(Vec::new());
		};

		let mut options: Vec<Cow<'static, str>> = vec![Cow::Borrowed("All")];
		let mut ids: Vec<Cow<'static, str>> = vec![Cow::Borrowed("all")];

		if let Some(elements) = Html::parse_fragment(genre_opt)?.select("option") {
			for element in elements {
				// option values look like "/genre/ne85/action"
				let Some(value) = element.attr("value") else {
					continue;
				};
				let Some(id) = value
					.split('/')
					.nth(2)
					.map(|id| id.trim_start_matches(char::is_alphabetic))
					.filter(|id| !id.is_empty())
				else {
					continue;
				};
				let Some(name) = element.text().filter(|name| !name.is_empty()) else {
					continue;
				};
				ids.push(id.to_string().into());
				options.push(name.into());
			}
		}

		Ok(vec![
			SelectFilter {
				id: "genre".into(),
				title: Some("Genre".into()),
				is_genre: true,
				options,
				ids: Some(ids),
				..Default::default()
			}
			.into(),
		])
	}
}

impl ImageRequestProvider for Rawdevart {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", format!("{BASE_URL}/").as_str()))
	}
}

impl DeepLinkHandler for Rawdevart {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};
		// shared urls tend to carry tracking parameters, which aren't part of the id
		let path = path.split(['?', '#']).next().unwrap_or_default();
		let segments = path
			.split('/')
			.filter(|segment| !segment.is_empty())
			.collect::<Vec<&str>>();

		Ok(match segments.as_slice() {
			// https://rawdevart.art/g/ne854721
			["g", id] => manga_key(id).map(|key| DeepLinkResult::Manga { key: key.into() }),
			// https://rawdevart.art/read/ne854721/chapter-28
			["read", id, chapter] | ["reader", id, chapter] => {
				manga_key(id).and_then(|manga_key| {
					let key = chapter.strip_prefix_or_self("chapter-");
					(!key.is_empty()).then(|| DeepLinkResult::Chapter {
						manga_key: manga_key.into(),
						key: key.into(),
					})
				})
			}
			_ => None,
		})
	}
}

register_source!(
	Rawdevart,
	ListingProvider,
	DynamicFilters,
	ImageRequestProvider,
	DeepLinkHandler
);

#[cfg(test)]
mod test {
	use super::*;
	use aidoku::FilterKind;
	use aidoku_test::aidoku_test;

	// "AR/MS!!", used as a stable entry to check parsing against
	const MANGA_KEY: &str = "854721";

	#[aidoku_test]
	fn test_listings() {
		for id in ["latest", "popular", "trending"] {
			let listing = Listing {
				id: id.into(),
				..Default::default()
			};
			let result = Rawdevart.get_manga_list(listing, 1).expect("listing");
			assert!(!result.entries.is_empty(), "{id} returned no entries");
			assert!(result.has_next_page, "{id} has no next page");

			let entry = &result.entries[0];
			assert!(!entry.key.is_empty());
			assert!(!entry.title.is_empty());
			assert!(entry.cover.is_some());
		}
	}

	#[aidoku_test]
	fn test_search() {
		let result = Rawdevart
			.get_search_manga_list(Some(String::from("isekai")), 1, Vec::new())
			.expect("search");
		assert!(!result.entries.is_empty());
	}

	#[aidoku_test]
	fn test_filters() {
		let filters = vec![
			FilterValue::Select {
				id: String::from("status"),
				value: String::from("completed"),
			},
			FilterValue::Sort {
				id: String::from("sort"),
				index: 1,
				ascending: false,
			},
		];
		let result = Rawdevart
			.get_search_manga_list(None, 1, filters)
			.expect("filtered list");
		assert!(!result.entries.is_empty());
	}

	#[aidoku_test]
	fn test_dynamic_filters() {
		let filters = Rawdevart.get_dynamic_filters().expect("dynamic filters");
		assert_eq!(filters.len(), 1);
		let FilterKind::Select { options, ids, .. } = &filters[0].kind else {
			panic!("expected a select filter");
		};
		// the site had 63 genres at the time of writing
		assert!(options.len() > 50);
		let ids = ids.as_ref().expect("genre ids");
		assert_eq!(ids.len(), options.len());
		// every id past the "all" default should be numeric
		assert!(
			ids[1..].iter().all(|id| id.parse::<i32>().is_ok()),
			"{ids:?}"
		);
	}

	#[aidoku_test]
	fn test_manga_details() {
		let manga = Manga {
			key: String::from(MANGA_KEY),
			..Default::default()
		};
		let manga = Rawdevart
			.get_manga_update(manga, true, true)
			.expect("manga details");

		assert_eq!(manga.title, "AR/MS!!");
		assert_eq!(
			manga.url.as_deref(),
			Some("https://rawdevart.art/g/ne854721")
		);
		assert!(manga.cover.is_some());
		assert!(manga.authors.is_some_and(|authors| !authors.is_empty()));
		assert!(manga.tags.is_some_and(|tags| !tags.is_empty()));
		assert!(manga.description.is_some_and(|it| !it.contains('<')));

		let chapters = manga.chapters.expect("chapters");
		assert!(!chapters.is_empty());
		let chapter = &chapters[0];
		assert!(!chapter.key.is_empty());
		assert!(chapter.chapter_number.is_some());
		// language stays unset so the app's chapter language filter can't hide these
		assert_eq!(chapter.language, None);
		// date_uploaded isn't checked here: the test runner doesn't implement the quoting,
		// fractional seconds or ISO 8601 zones that DATE_FORMAT relies on, so it only ever
		// parses on device
	}

	// most chapters are numbered like "34.2", so the keys built from them have to survive
	// the round trip back into a page request
	#[aidoku_test]
	fn test_decimal_chapter_pages() {
		let manga = Manga {
			key: String::from("16523"),
			..Default::default()
		};
		// called the same way the app does when opening a manga page
		let manga = Rawdevart
			.get_manga_update(manga, true, true)
			.expect("chapters");
		let chapters = manga.chapters.clone().expect("chapters");

		let chapter = chapters
			.iter()
			.find(|chapter| {
				chapter
					.chapter_number
					.is_some_and(|number| number.fract() != 0.0)
			})
			.expect("a decimal chapter")
			.clone();

		let pages = Rawdevart
			.get_page_list(manga, chapter)
			.expect("decimal chapter pages");
		assert!(!pages.is_empty(), "decimal chapter returned no pages");
	}

	#[aidoku_test]
	fn test_page_list() {
		let manga = Manga {
			key: String::from(MANGA_KEY),
			..Default::default()
		};
		let chapter = Chapter {
			key: String::from("1"),
			..Default::default()
		};
		let pages = Rawdevart.get_page_list(manga, chapter).expect("page list");

		assert!(!pages.is_empty());
		for page in &pages {
			let PageContent::Url(url, _) = &page.content else {
				panic!("expected a page url");
			};
			assert!(url.starts_with("http"), "{url} is not absolute");
		}
	}

	#[aidoku_test]
	fn test_deep_link() {
		let manga = Rawdevart
			.handle_deep_link(String::from("https://rawdevart.art/g/ne854721"))
			.expect("manga deep link");
		assert_eq!(
			manga,
			Some(DeepLinkResult::Manga {
				key: String::from(MANGA_KEY)
			})
		);

		let chapter = Rawdevart
			.handle_deep_link(String::from(
				"https://rawdevart.art/read/ne854721/chapter-28",
			))
			.expect("chapter deep link");
		assert_eq!(
			chapter,
			Some(DeepLinkResult::Chapter {
				manga_key: String::from(MANGA_KEY),
				key: String::from("28")
			})
		);

		let unknown = Rawdevart
			.handle_deep_link(String::from("https://rawdevart.art/latest"))
			.expect("unknown deep link");
		assert_eq!(unknown, None);

		// shared urls often carry tracking parameters, which aren't part of the id
		let tracked = Rawdevart
			.handle_deep_link(String::from(
				"https://rawdevart.art/g/ne854721?utm_source=share#top",
			))
			.expect("tracked deep link");
		assert_eq!(
			tracked,
			Some(DeepLinkResult::Manga {
				key: String::from(MANGA_KEY)
			})
		);

		// anything that doesn't leave a numeric id behind isn't a manga link
		for url in [
			"https://rawdevart.art/g/abc",
			"https://rawdevart.art/g/",
			"https://rawdevart.art/read/ne854721/chapter-",
		] {
			assert_eq!(
				Rawdevart.handle_deep_link(String::from(url)).expect(url),
				None,
				"{url} should not resolve"
			);
		}
	}

	// the entry that was reported as having no readable chapters
	#[aidoku_test]
	fn test_reported_manga() {
		let manga = Manga {
			key: String::from("858338"),
			..Default::default()
		};
		let manga = Rawdevart
			.get_manga_update(manga, true, true)
			.expect("reported manga");

		// bounds rather than exact counts, since the entry is still being updated
		let chapters = manga.chapters.clone().expect("chapters");
		assert!(chapters.len() >= 9, "only {} chapters", chapters.len());
		assert!(chapters.iter().all(|chapter| chapter.language.is_none()));

		let chapter = chapters
			.iter()
			.find(|chapter| chapter.key == "8")
			.expect("chapter 8")
			.clone();
		assert_eq!(
			chapter.url.as_deref(),
			Some("https://rawdevart.art/read/ne858338/chapter-8")
		);

		let pages = Rawdevart
			.get_page_list(manga, chapter)
			.expect("reported chapter pages");
		assert!(pages.len() >= 20, "only {} pages", pages.len());
		for page in &pages {
			let PageContent::Url(url, _) = &page.content else {
				panic!("expected a page url");
			};
			assert!(url.starts_with("https://"), "{url} is not absolute");
			assert!(!url.contains("//data/"), "{url} has a broken join");
		}
	}
}
