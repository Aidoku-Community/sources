#![no_std]

mod helpers;
mod home;
mod settings;

use aidoku::{
	BasicLoginHandler, Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue,
	HashMap, Home as HomeProvider, HomeComponent, HomeComponentValue, HomeLayout,
	ImageRequestProvider, Link, Listing, ListingProvider, Manga, MangaPageResult, MangaStatus,
	NotificationHandler, Page, PageContent, PageContext, Result, Source, Viewer,
	alloc::{String, Vec, format},
	imports::{
		defaults::{DefaultValue, defaults_get, defaults_set},
		net::Request,
		std::send_partial_result,
	},
	prelude::*,
};
use serde_json::{Value, json};

const BASE_URL: &str = "https://komiic.com";
const QUERY_URL: &str = "https://komiic.com/api/query";
const LOGIN_URL: &str = "https://komiic.com/api/login";
const IMAGE_URL: &str = "https://komiic.com/api/image";
const REFERER_URL: &str = "https://komiic.com/";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
const PAGE_SIZE: i32 = 20;
const CATEGORY_PAGE_SIZE: i32 = 30;
const RECOMMENDATION_PAGE_SIZE: i32 = 25;
const SORT_ORDER_IDS: [&str; 3] = ["DATE_UPDATED", "VIEWS", "FAVORITE_COUNT"];
const PREFER_BOOKS_KEY: &str = "preferBooks";
const TOKEN_KEY: &str = "token";
const JUST_LOGGED_IN_KEY: &str = "justLoggedIn";

const COMIC_FIELDS: &str = r#"
id
title
status
imageUrl
__typename
"#;

const HOME_COMIC_FIELDS: &str = r#"
id
title
status
imageUrl
authors { id name __typename }
categories { id name __typename }
__typename
"#;

const DETAIL_COMIC_FIELDS: &str = r#"
id
title
status
year
imageUrl
authors { id name __typename }
categories { id name __typename }
dateCreated
dateUpdated
monthViews
views
favoriteCount
lastBookUpdate
lastChapterUpdate
description
__typename
"#;

struct KomiicSource;

impl KomiicSource {
	fn chapter_title(value: &Value) -> Option<String> {
		let serial = value.get("serial").and_then(Value::as_str)?.trim();
		if serial.is_empty() {
			return None;
		}
		if serial.parse::<f32>().is_ok() {
			return None;
		}
		if value.get("type").and_then(Value::as_str) == Some("book") {
			Some(format!("卷{serial}"))
		} else {
			Some(String::from(serial))
		}
	}

	fn parse_chapter(value: &Value, manga_key: &str) -> Option<Chapter> {
		let key = Self::string_field(value, "id")?;
		let serial = value
			.get("serial")
			.and_then(Value::as_str)
			.unwrap_or_default();
		let serial_number = serial.parse::<f32>().ok();
		let is_book = value.get("type").and_then(Value::as_str) == Some("book");
		let url = format!("{BASE_URL}/comic/{manga_key}/chapter/{key}/images/all");
		Some(Chapter {
			key,
			title: Self::chapter_title(value),
			chapter_number: if is_book { None } else { serial_number },
			volume_number: if is_book { serial_number } else { None },
			url: Some(url),
			language: Some(String::from("zh")),
			..Default::default()
		})
	}

	fn select_chapter_version(
		books: Vec<Chapter>,
		web_chapters: Vec<Chapter>,
		prefer_books: bool,
	) -> Vec<Chapter> {
		if prefer_books && !books.is_empty() {
			books
		} else if !web_chapters.is_empty() {
			web_chapters
		} else {
			books
		}
	}

	fn chapters(manga_key: &str) -> Result<Vec<Chapter>> {
		let json = Self::query(json!({
			"operationName": "chapterByComicId",
			"variables": { "comicId": manga_key },
			"query": "query chapterByComicId($comicId: ID!) { chaptersByComicId(comicId: $comicId) { id serial type dateCreated dateUpdated size __typename } }"
		}))?;
		let values = json
			.get("data")
			.and_then(|value| value.get("chaptersByComicId"))
			.and_then(Value::as_array)
			.ok_or_else(|| error!("Komiic missing chapters"))?;
		let mut books = Vec::new();
		let mut web_chapters = Vec::new();
		for value in values {
			if let Some(chapter) = Self::parse_chapter(value, manga_key) {
				if value.get("type").and_then(Value::as_str) == Some("book") {
					books.push(chapter);
				} else {
					web_chapters.push(chapter);
				}
			}
		}
		let mut chapters = Self::select_chapter_version(books, web_chapters, Self::prefers_books());
		chapters.sort_by(|a, b| {
			let a_number = a.chapter_number.or(a.volume_number).unwrap_or(0.0);
			let b_number = b.chapter_number.or(b.volume_number).unwrap_or(0.0);
			b_number
				.partial_cmp(&a_number)
				.unwrap_or(core::cmp::Ordering::Equal)
		});
		Ok(chapters)
	}
}

impl Source for KomiicSource {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let (order_by, status, category, keyword, author) =
			Self::parse_search_filters(query.as_deref(), &filters);
		if let Some(author) = author {
			Self::search_by_author(author, page)
		} else if let Some(keyword) = keyword {
			if let Some(category_id) = Self::category_filter_value(keyword.as_str()) {
				Self::comics_by_category(
					category_id.as_str(),
					order_by.as_str(),
					status.as_str(),
					page,
				)
			} else {
				Self::search(keyword)
			}
		} else {
			Self::comics_by_category(category.as_str(), order_by.as_str(), status.as_str(), page)
		}
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			if let Some(updated) = Self::comic_by_id(manga.key.clone())? {
				manga.copy_from(updated);
			}
			if needs_chapters {
				send_partial_result(&manga);
			}
		}
		if needs_chapters {
			manga.chapters = Some(Self::chapters(&manga.key)?);
		}
		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let json = Self::query(json!({
			"operationName": "imagesByChapterId",
			"variables": { "chapterId": chapter.key },
			"query": "query imagesByChapterId($chapterId: ID!) { imagesByChapterId(chapterId: $chapterId) { id kid height width __typename } }"
		}))?;
		let values = json
			.get("data")
			.and_then(|value| value.get("imagesByChapterId"))
			.and_then(Value::as_array)
			.ok_or_else(|| error!("Komiic missing images"))?;
		let mut pages = Vec::new();
		for value in values {
			if let Some(kid) = value.get("kid").and_then(Value::as_str) {
				let mut context: PageContext = HashMap::new();
				context.insert(
					String::from("referer"),
					format!(
						"{BASE_URL}/comic/{}/chapter/{}/images/all",
						manga.key, chapter.key
					),
				);
				pages.push(Page {
					content: PageContent::url_context(format!("{IMAGE_URL}/{kid}"), context),
					..Default::default()
				});
			}
		}
		Ok(pages)
	}
}

impl ListingProvider for KomiicSource {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		match listing.id.as_str() {
			"recent_update" => Self::get_comic_list("recentUpdate", "DATE_UPDATED", page),
			"recommendations" => Self::recommendations(page),
			"month_views" => Self::get_comic_list("hotComics", "MONTH_VIEWS", page),
			"views" => Self::get_comic_list("hotComics", "VIEWS", page),
			id => Self::category_listing(id, page).unwrap_or_else(|| Ok(Self::empty_page())),
		}
	}
}

impl DeepLinkHandler for KomiicSource {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let mut splits = url.split('/').skip(3);
		let result = match splits.next() {
			Some("comic") => match (splits.next(), splits.next(), splits.next(), splits.next()) {
				(Some(key), None, None, None) => Some(DeepLinkResult::Manga { key: key.into() }),
				(Some(manga_key), Some("chapter"), Some(key), _) => Some(DeepLinkResult::Chapter {
					manga_key: manga_key.into(),
					key: key.into(),
				}),
				_ => None,
			},
			_ => None,
		};
		Ok(result)
	}
}

impl ImageRequestProvider for KomiicSource {
	fn get_image_request(
		&self,
		url: String,
		context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		let referer = context
			.and_then(|value| value.get("referer").cloned())
			.unwrap_or_else(|| String::from(REFERER_URL));
		let mut request = Request::get(url)?;
		request.set_header("User-Agent", USER_AGENT);
		request.set_header("Referer", referer.as_str());
		if let Some(token) = Self::auth_token() {
			let authorization = format!("Bearer {token}");
			request.set_header("Authorization", authorization.as_str());
		}
		Ok(request)
	}
}

register_source!(
	KomiicSource,
	ListingProvider,
	Home,
	DeepLinkHandler,
	ImageRequestProvider,
	BasicLoginHandler,
	NotificationHandler
);

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn recent_update_returns_entries() {
		let source = KomiicSource;
		let result = source
			.get_manga_list(
				Listing {
					id: String::from("recent_update"),
					name: String::from("最近更新"),
					..Default::default()
				},
				1,
			)
			.expect("recent update request should succeed");

		assert!(
			!result.entries.is_empty(),
			"recent update should return entries"
		);
	}

	#[aidoku_test]
	fn home_returns_components() {
		let source = KomiicSource;
		let result = source.get_home().expect("home request should succeed");

		assert!(
			result.components.len() >= 3,
			"home should return multiple components"
		);
	}

	#[aidoku_test]
	fn sort_filter_uses_index_value() {
		let filters = Vec::from([FilterValue::Sort {
			id: String::from("sort"),
			index: 2,
			ascending: false,
		}]);

		let (order_by, _, _, _, _) = KomiicSource::parse_search_filters(None, &filters);

		assert_eq!(order_by, "FAVORITE_COUNT");
	}

	#[aidoku_test]
	fn author_filter_uses_author_search() {
		let filters = Vec::from([FilterValue::Text {
			id: String::from("author"),
			value: String::from("米二"),
		}]);

		let (_, _, _, keyword, author) = KomiicSource::parse_search_filters(None, &filters);

		assert_eq!(keyword, None);
		assert_eq!(author.as_deref(), Some("米二"));
	}

	#[aidoku_test]
	fn author_search_returns_author_works() {
		let source = KomiicSource;
		let result = source
			.get_search_manga_list(
				None,
				1,
				Vec::from([FilterValue::Text {
					id: String::from("author"),
					value: String::from("米二"),
				}]),
			)
			.expect("author search should succeed");

		assert!(
			result.entries.iter().any(|entry| entry.title == "一人之下"),
			"author search should use Komiic author results"
		);
	}

	#[aidoku_test]
	fn manga_list_reports_raw_count_before_deduplication() {
		let value = serde_json::json!({
			"data": {
				"comics": [
					{ "id": "1", "title": "A" },
					{ "id": "1", "title": "A duplicate" }
				]
			}
		});

		let (entries, raw_count) =
			KomiicSource::parse_manga_list_with_raw_count(&value, &["data", "comics"])
				.expect("list should parse");

		assert_eq!(raw_count, 2);
		assert_eq!(entries.len(), 1);
	}

	#[aidoku_test]
	fn manga_list_uses_minimal_fields() {
		let value = serde_json::json!({
			"data": {
				"comics": [
					{
						"id": "1",
						"title": "A",
						"status": "ONGOING",
						"imageUrl": "https://example.com/cover.jpg",
						"description": "detail-only",
						"authors": [
							{ "name": "作者A" },
							{ "name": "作者B" }
						],
						"categories": [
							{ "name": "熱血" }
						]
					}
				]
			}
		});

		let (entries, _) =
			KomiicSource::parse_manga_list_with_raw_count(&value, &["data", "comics"])
				.expect("list should parse");

		let manga = &entries[0];
		assert_eq!(manga.key, "1");
		assert_eq!(manga.title, "A");
		assert_eq!(
			manga.cover.as_deref(),
			Some("https://example.com/cover.jpg")
		);
		assert_eq!(manga.url.as_deref(), Some("https://komiic.com/comic/1"));
		assert_eq!(manga.status, MangaStatus::Ongoing);
		assert_eq!(manga.content_rating, ContentRating::Safe);
		assert_eq!(manga.viewer, Viewer::RightToLeft);
		assert_eq!(manga.authors, None);
		assert_eq!(manga.description, None);
		assert_eq!(manga.tags, None);
	}

	#[aidoku_test]
	fn home_manga_list_preserves_authors_and_tags() {
		let value = serde_json::json!({
			"data": {
				"comics": [
					{
						"id": "1",
						"title": "A",
						"authors": [
							{ "name": "作者A" },
							{ "name": "作者B" }
						],
						"categories": [
							{ "name": "熱血" },
							{ "name": "冒險" }
						]
					}
				]
			}
		});

		let (entries, _) = KomiicSource::parse_manga_list_with_raw_count_and_mode(
			&value,
			&["data", "comics"],
			false,
		)
		.expect("home list should parse");

		assert_eq!(
			entries[0].authors,
			Some(Vec::from([String::from("作者A"), String::from("作者B")]))
		);
		assert_eq!(
			entries[0].tags,
			Some(Vec::from([String::from("熱血"), String::from("冒險")]))
		);
	}

	#[aidoku_test]
	fn recommendations_require_login() {
		let source = KomiicSource;

		let result = source.get_manga_list(
			Listing {
				id: String::from("recommendations"),
				name: String::from("个性化推荐"),
				..Default::default()
			},
			1,
		);

		assert!(result.is_err(), "recommendations should require login");
	}

	#[aidoku_test]
	fn category_listing_returns_entries() {
		let source = KomiicSource;
		let result = source
			.get_manga_list(
				Listing {
					id: String::from("category:1:DATE_UPDATED"),
					name: String::from("愛情"),
					..Default::default()
				},
				1,
			)
			.expect("category listing request should succeed");

		assert!(
			!result.entries.is_empty(),
			"category listing should return entries"
		);
	}

	#[aidoku_test]
	fn detail_returns_description_and_clean_chapter_titles() {
		let source = KomiicSource;
		defaults_set(PREFER_BOOKS_KEY, DefaultValue::Bool(true));
		let manga = source
			.get_manga_update(
				Manga {
					key: String::from("2100"),
					..Default::default()
				},
				true,
				true,
			)
			.expect("detail request should succeed");

		let description = manga
			.description
			.expect("detail should include description");
		assert!(
			description.contains("阿波羅"),
			"description should come from comicById"
		);

		let chapters = manga.chapters.expect("detail should include chapters");
		assert!(
			chapters
				.iter()
				.all(|chapter| chapter.volume_number.is_some() && chapter.chapter_number.is_none()),
			"default chapter list should only include volumes"
		);
		assert!(
			chapters
				.iter()
				.any(|chapter| chapter.volume_number == Some(9.0)),
			"expected known volume 9"
		);
		assert!(
			chapters
				.iter()
				.filter(
					|chapter| chapter.chapter_number.is_some() || chapter.volume_number.is_some()
				)
				.all(|chapter| chapter.title.is_none()),
			"numeric serials should not be repeated as chapter titles"
		);
	}

	#[aidoku_test]
	fn chapter_version_selection_matches_preference() {
		let books = Vec::from([Chapter {
			key: String::from("book-1"),
			volume_number: Some(1.0),
			..Default::default()
		}]);
		let web_chapters = Vec::from([Chapter {
			key: String::from("chapter-1"),
			chapter_number: Some(1.0),
			..Default::default()
		}]);

		let selected =
			KomiicSource::select_chapter_version(books.clone(), web_chapters.clone(), true);
		assert_eq!(selected[0].key, "book-1");

		let selected =
			KomiicSource::select_chapter_version(books.clone(), web_chapters.clone(), false);
		assert_eq!(selected[0].key, "chapter-1");

		let selected = KomiicSource::select_chapter_version(Vec::new(), web_chapters, true);
		assert_eq!(selected[0].key, "chapter-1");

		let selected = KomiicSource::select_chapter_version(books, Vec::new(), false);
		assert_eq!(selected[0].key, "book-1");
	}
}
