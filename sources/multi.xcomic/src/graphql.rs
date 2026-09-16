use crate::{
	helpers::{editions_of, is_pornographic},
	models::{
		BrowseResponse, ChapterData, ChapterListResponse, ChapterPagesResponse, ComicData,
		ComicNodeResponse, GraphQlResponse, LatestUploadsResponse, RecentlyAddedResponse,
	},
	settings,
};
use aidoku::{
	Result,
	alloc::{String, Vec, format, string::ToString, vec},
	imports::net::{Request, Response},
	prelude::*,
};
use serde::de::DeserializeOwned;

pub const PAGE_SIZE: i32 = 48;
/// Each title here carries up to three chapters, so a browse page is overkill.
pub const HOME_LATEST_SIZE: i32 = 20;
const RECENTLY_ADDED_SIZE: i32 = 50;
const RANDOM_AMOUNT: i32 = 12;
const CHAPTER_PAGE_SIZE: i32 = 100;
const UNIQUE_CHAPTER_PAGE_SIZE: i32 = 1000;

pub const SORT_IDS: &[&str] = &[
	"field_score",
	"field_update",
	"field_create",
	"field_name_asc",
	"field_name_desc",
	"field_follow",
	"field_review",
	"field_comment",
	"field_chapter",
];

// Browse is keyed by title; a title carries its editions as `comicNodes`, which keeps
// the key a comic id.
const BROWSE_QUERY: &str = r#"
query get_title_browse_items($select: Title_Browse_Select) {
  items: get_title_browse_items(select: $select) {
    data {
      name: title
      urlCover: cover_local_url
      contentRating: content_rating_id
      originalStatus: status
    }
    comicNodes { data { id name subName urlPath translatedLanguage chaps_normal } }
  }
}
"#;

// The big scroller is the one component that renders a description and tags.
const SCROLLER_QUERY: &str = r#"
query get_title_browse_items($select: Title_Browse_Select) {
  items: get_title_browse_items(select: $select) {
    data {
      name: title
      urlCover: cover_local_url
      contentRating: content_rating_id
      originalStatus: status
      genres: genre_ids
      description
    }
    comicNodes { data { id name subName urlPath translatedLanguage chaps_normal } }
  }
}
"#;

// A random title answers a narrower set than browse does.
const RANDOM_QUERY: &str = r#"
query get_title_randomList($select: Title_RandomList_Select) {
  items: get_title_randomList(select: $select) {
    data {
      urlCover: cover_local_url
      contentRating: content_rating_id
    }
    comicNodes { data { id name subName urlPath translatedLanguage chaps_normal } }
  }
}
"#;

const RECENTLY_ADDED_QUERY: &str = r#"
query get_title_recentlyAdded($select: Title_RecentlyAdded_Select) {
  get_title_recentlyAdded(select: $select) {
    items {
      data {
        id
        name: title
        urlCover: cover_local_url
        contentRating: content_rating_id
        type: type_id
        genres: genre_ids
      }
      comicNodes { data { id name subName urlPath translatedLanguage chaps_normal } }
    }
  }
}
"#;

// Three chapters per title reach its other editions.
const LATEST_UPLOADS_QUERY: &str = r#"
query get_title_latestUploads($select: Title_LatestUploads_Select) {
  get_title_latestUploads(select: $select) {
    items {
      chapters(amount: 3) {
        data {
          id serial chaNum dname urlPath dbStatus
          datePublic dateCreate dateModify
          comicNode {
            data {
              id name subName urlPath urlCover translatedLanguage
              type contentRating genres
            }
          }
        }
      }
    }
  }
}
"#;

const COMIC_QUERY: &str = r#"
query get_comicNode($id: ID!) {
  get_comicNode(id: $id) {
    data {
      id name subName type demographics contentRating genres tags
      originalStatus uploadStatus readDirection translatedLanguage
      authorNodes { data { name } }
      artistNodes { data { name } }
      tagNodes { data { name } }
      summary { text }
      urlPath urlCover
    }
  }
}
"#;

// The site collapses alternate uploads of a chapter into one on request. Both
// forms select the same fields, and are aliased so one model reads either.
const CHAPTERS_QUERY: &str = r#"
query get_comic_chapterList_fullList($select: Select_Comic_ChapterList) {
  chapterList: get_comic_chapterList_fullList(select: $select) {
    paging { next total }
    items {
      data {
        id dbStatus serial chaNum volNum dname title urlPath
        dateCreate dateModify datePublic srcName
        profileNodes { data { name } }
        groupNodes { data { name } }
      }
    }
  }
}
"#;

const UNIQUE_CHAPTERS_QUERY: &str = r#"
query get_comic_chapterList_uniqList($select: Select_Comic_ChapterList_UniqList) {
  chapterList: get_comic_chapterList_uniqList(select: $select) {
    paging { next total }
    items {
      data {
        id dbStatus serial chaNum volNum dname title urlPath
        dateCreate dateModify datePublic srcName
        profileNodes { data { name } }
        groupNodes { data { name } }
      }
    }
  }
}
"#;

const CHAPTER_PAGES_QUERY: &str = r#"
query get_chapterNode($id: ID!) {
  get_chapterNode(id: $id) { data { imageUrls } }
}
"#;

#[derive(Default)]
pub struct BrowseParams {
	pub page: i32,
	pub size: i32,
	pub sortby: String,
	pub word: String,
	pub included_genres: Vec<String>,
	pub excluded_genres: Vec<String>,
	pub include_mode: String,
	pub exclude_mode: String,
	pub types: Vec<String>,
	pub demographics: Vec<String>,
	pub content_ratings: Vec<String>,
	pub original_languages: Vec<String>,
	pub translated_languages: Vec<String>,
	pub original_status: String,
	pub chapter_count: String,
	pub year_min: Option<i64>,
	pub year_max: Option<i64>,
}

impl BrowseParams {
	pub fn new(sortby: &str, page: i32, size: i32) -> Self {
		Self {
			page,
			size,
			sortby: sortby.into(),
			include_mode: "and".into(),
			exclude_mode: "or".into(),
			excluded_genres: settings::excluded_genres(),
			types: settings::content_types(),
			content_ratings: settings::content_ratings(),
			translated_languages: settings::languages(),
			..Default::default()
		}
	}

	fn allows_pornographic(&self) -> bool {
		self.content_ratings
			.iter()
			.any(|value| value == "pornographic")
	}

	fn select(&self) -> serde_json::Value {
		// An empty list is not "every rating" here: the site answers one with its own
		// safe default, which drops everything erotica and above.
		let content_ratings: Vec<&str> = if self.content_ratings.is_empty() {
			vec!["safe"]
		} else {
			self.content_ratings.iter().map(String::as_str).collect()
		};
		let excluded_genres: Vec<&str> = self.excluded_genres.iter().map(String::as_str).collect();
		let types: &[String] = if self.types.len() >= settings::CONTENT_TYPES.len() {
			&[]
		} else {
			&self.types
		};
		let mut select = serde_json::json!({
			"where": "browse",
			"page": self.page,
			"size": self.size,
			"sortby": self.sortby,
			"incOLangs": self.original_languages,
			"incTLangs": self.translated_languages,
			"incGenres": self.included_genres,
			"excGenres": excluded_genres,
			"incGenresMode": self.include_mode,
			"excGenresMode": self.exclude_mode,
			"incTypes": types,
			"incDemographics": self.demographics,
			"incContentRatings": content_ratings,
			"releaseYearMin": self.year_min,
			"releaseYearMax": self.year_max,
			"origStatus": (!self.original_status.is_empty()).then_some(&self.original_status),
			"chapCount": (!self.chapter_count.is_empty()).then_some(&self.chapter_count),
			// Off, as the site sends them: its blocklist hides unapproved uploads.
			"ignoreGlobalULangs": false,
			"ignoreGlobalGenres": false,
			"ignoreGlobalBlocks": false
		});
		if !self.word.is_empty() {
			select["word"] = self.word.as_str().into();
		}
		select
	}

	/// Only the two feeds need this: they take no filters of their own, where
	/// browse applies every one of them server side.
	fn allows(&self, comic: &ComicData) -> bool {
		let rating = comic.content_rating.as_deref().unwrap_or("safe");
		let genres = comic.genres.as_deref().unwrap_or_default();
		(self.translated_languages.is_empty()
			|| comic
				.translated_language
				.as_ref()
				.is_some_and(|language| self.translated_languages.contains(language)))
			// An undeclared type is common on new uploads, and is no reason to hide one.
			&& comic
				.kind
				.as_deref()
				.is_none_or(|kind| self.types.iter().any(|value| value == kind))
			&& self.content_ratings.iter().any(|value| value == rating)
			&& !genres
				.iter()
				.any(|genre| self.excluded_genres.contains(genre))
			&& (self.allows_pornographic()
				|| !is_pornographic(comic.content_rating.as_deref(), comic.genres.as_deref()))
	}
}

fn graphql_request(base_url: &str, query: &str, variables: serde_json::Value) -> Result<Request> {
	let languages = settings::languages();
	let accept_language = if languages.is_empty() {
		"en".into()
	} else {
		languages
			.iter()
			.map(|language| language.replace('_', "-"))
			.collect::<Vec<_>>()
			.join(",")
	};
	Ok(Request::post(format!("{base_url}/query/"))?
		.header("Content-Type", "application/json")
		.header("Accept", "application/json")
		.header("Accept-Language", &accept_language)
		.header("Referer", &format!("{base_url}/"))
		.header("Origin", base_url)
		.body(serde_json::json!({ "query": query.trim(), "variables": variables }).to_string()))
}

fn parse_graphql<T: DeserializeOwned>(response: Response) -> Result<T> {
	let response: GraphQlResponse<T> = response.get_json_owned()?;
	if let Some(data) = response.data {
		Ok(data)
	} else if let Some(error) = response.errors.into_iter().next() {
		bail!("XCOMIC: {}", error.message);
	} else {
		bail!("XCOMIC returned an empty response");
	}
}

fn graphql<T: DeserializeOwned>(
	base_url: &str,
	query: &str,
	variables: serde_json::Value,
) -> Result<T> {
	parse_graphql(graphql_request(base_url, query, variables)?.send()?)
}

pub fn browse_request(base_url: &str, params: &BrowseParams) -> Result<Request> {
	graphql_request(
		base_url,
		BROWSE_QUERY,
		serde_json::json!({ "select": params.select() }),
	)
}

pub fn scroller_request(base_url: &str, params: &BrowseParams) -> Result<Request> {
	graphql_request(
		base_url,
		SCROLLER_QUERY,
		serde_json::json!({ "select": params.select() }),
	)
}

pub fn random_request(base_url: &str) -> Result<Request> {
	graphql_request(
		base_url,
		RANDOM_QUERY,
		serde_json::json!({ "select": { "amount": RANDOM_AMOUNT } }),
	)
}

pub fn recently_added_request(base_url: &str) -> Result<Request> {
	graphql_request(
		base_url,
		RECENTLY_ADDED_QUERY,
		serde_json::json!({ "select": { "size": RECENTLY_ADDED_SIZE } }),
	)
}

pub fn parse_recently_added(response: Response, params: &BrowseParams) -> Result<Vec<ComicData>> {
	let response: RecentlyAddedResponse = parse_graphql(response)?;
	Ok(response
		.recently_added
		.unwrap_or_default()
		.items
		.unwrap_or_default()
		.into_iter()
		.flat_map(|title| editions_of(title, &params.translated_languages, false))
		.filter(|comic| params.allows(comic))
		.collect())
}

pub fn latest_uploads_request(base_url: &str, limit: i32) -> Result<Request> {
	graphql_request(
		base_url,
		LATEST_UPLOADS_QUERY,
		serde_json::json!({ "select": { "first": 0, "limit": limit } }),
	)
}

pub fn parse_latest_uploads(
	response: Response,
	params: &BrowseParams,
) -> Result<Vec<(ComicData, ChapterData)>> {
	let response: LatestUploadsResponse = parse_graphql(response)?;

	// Items are titles, and arrive grouped by title rather than newest first.
	let mut chapters: Vec<ChapterData> = response
		.latest_uploads
		.unwrap_or_default()
		.items
		.into_iter()
		.flatten()
		.filter_map(|item| item.chapters)
		.flatten()
		.map(|node| node.data)
		.filter(|chapter| chapter.db_status.as_deref().unwrap_or("normal") == "normal")
		.collect();
	chapters.sort_by_key(|chapter| {
		core::cmp::Reverse(
			chapter
				.date_public
				.or(chapter.date_modify)
				.or(chapter.date_create)
				.unwrap_or_default(),
		)
	});

	let mut seen: Vec<String> = Vec::new();
	let comics = chapters
		.into_iter()
		.filter_map(|chapter| {
			let comic = chapter.comic_node.and_then(|node| node.data)?;
			let chapter = ChapterData {
				comic_node: None,
				..chapter
			};
			params.allows(&comic).then_some((comic, chapter))
		})
		// Three chapters per title can share a comic, so keep only its newest.
		.filter(|(comic, _)| {
			let unseen = !seen.contains(&comic.id);
			if unseen {
				seen.push(comic.id.clone());
			}
			unseen
		})
		.collect();
	Ok(comics)
}

pub fn parse_titles(response: Response, params: &BrowseParams) -> Result<(Vec<ComicData>, usize)> {
	let response: BrowseResponse = parse_graphql(response)?;
	let titles = response.items.unwrap_or_default();
	let served = titles.len();
	let comics = titles
		.into_iter()
		.flat_map(|title| editions_of(title, &params.translated_languages, !params.word.is_empty()))
		.collect();
	Ok((comics, served))
}

pub fn parse_browse(response: Response, params: &BrowseParams) -> Result<(Vec<ComicData>, bool)> {
	let (comics, served) = parse_titles(response, params)?;
	// Counted over the titles served, which the cards may expand past.
	Ok((comics, served as i32 >= params.size))
}

pub fn fetch_comic(base_url: &str, id: &str) -> Result<ComicData> {
	let response: ComicNodeResponse =
		graphql(base_url, COMIC_QUERY, serde_json::json!({ "id": id }))?;
	response
		.comic
		.map(|node| node.data)
		.ok_or_else(|| error!("Manga not found"))
}

pub fn fetch_chapters(base_url: &str, comic_id: &str) -> Result<Vec<ChapterData>> {
	// The full list carries every scanlator's upload, so it is paged smaller.
	let (query, size) = if settings::deduplicate_chapters() {
		(UNIQUE_CHAPTERS_QUERY, UNIQUE_CHAPTER_PAGE_SIZE)
	} else {
		(CHAPTERS_QUERY, CHAPTER_PAGE_SIZE)
	};
	let first: ChapterListResponse =
		graphql(base_url, query, chapter_variables(comic_id, 1, size))?;
	let paging = first
		.chapter_list
		.as_ref()
		.and_then(|result| result.paging.as_ref());
	let total = paging.and_then(|paging| paging.total);
	let has_next = paging.and_then(|paging| paging.next).unwrap_or_default() != 0;
	let mut chapters: Vec<ChapterData> = first
		.chapter_list
		.map(|result| {
			result
				.items
				.unwrap_or_default()
				.into_iter()
				.map(|node| node.data)
				.collect()
		})
		.unwrap_or_default();

	let total = total.unwrap_or(chapters.len() as i64);
	let pages = if has_next && total > size as i64 {
		(total + size as i64 - 1) / size as i64
	} else {
		1
	};
	// More than three pages at once and the site answers 429, losing the list.
	let remaining: Vec<i64> = (2..=pages).collect();
	for batch in remaining.chunks(3) {
		let requests = batch
			.iter()
			.map(|page| {
				graphql_request(
					base_url,
					query,
					chapter_variables(comic_id, *page as i32, size),
				)
			})
			.collect::<Result<Vec<Request>>>()?;
		for response in Request::send_all(requests) {
			let response: ChapterListResponse = parse_graphql(response?)?;
			if let Some(result) = response.chapter_list {
				chapters.extend(
					result
						.items
						.unwrap_or_default()
						.into_iter()
						.map(|node| node.data),
				);
			}
		}
	}
	Ok(chapters)
}

fn chapter_variables(comic_id: &str, page: i32, size: i32) -> serde_json::Value {
	serde_json::json!({
		"select": {
			"comic_id": comic_id,
			"page": page,
			"size": size,
			"sortby": "chapter_desc"
		}
	})
}

pub fn fetch_page_urls(base_url: &str, chapter_id: &str) -> Result<Vec<String>> {
	let response: ChapterPagesResponse = graphql(
		base_url,
		CHAPTER_PAGES_QUERY,
		serde_json::json!({ "id": chapter_id }),
	)?;
	Ok(response
		.chapter
		.and_then(|node| node.data)
		.and_then(|data| data.image_urls)
		.unwrap_or_default())
}
