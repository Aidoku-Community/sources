use crate::{
	helpers::{PORNOGRAPHIC_GENRES, is_pornographic},
	models::{
		BrowseResponse, ChapterData, ChapterListResponse, ChapterPagesResponse, ComicData,
		ComicNodeResponse, GraphQlResponse, LatestEntry, LatestUploadsResponse,
		LatestUploadsResult, RecentlyAddedResponse,
	},
	settings,
};
use aidoku::{
	Result,
	alloc::{String, Vec, string::ToString},
	imports::net::{Request, Response},
	prelude::*,
};
use serde::de::DeserializeOwned;

pub const PAGE_SIZE: i32 = 48;
const RECENTLY_ADDED_SIZE: i32 = 50;
const CHAPTER_PAGE_SIZE: i32 = 100;
const UNIQUE_CHAPTER_PAGE_SIZE: i32 = 1000;

pub const SORT_IDS: &[&str] = &[
	"field_score",
	"field_update",
	"field_create",
	"field_name_asc",
	"field_name_desc",
	"field_chapter",
	"field_follow",
	"field_review",
	"field_comment",
	"views_d000",
	"views_d360",
	"views_d180",
	"views_d090",
	"views_d030",
	"views_d007",
	"views_h024",
	"views_h012",
	"views_h006",
	"views_h001",
];

// Browse takes every filter server side, so a search card needs nothing beyond
// what it displays; `get_manga_update` fills in the rest.
const BROWSE_QUERY: &str = r#"
query get_comic_browse_items($select: Comic_Browse_Select) {
  get_comic_browse_items(select: $select) {
    data {
      id name urlPath urlCover
      contentRating originalStatus uploadStatus
    }
  }
}
"#;

// The big scroller is the one component that renders a description and tags.
const SCROLLER_QUERY: &str = r#"
query get_comic_browse_items($select: Comic_Browse_Select) {
  get_comic_browse_items(select: $select) {
    data {
      id name urlPath urlCover
      contentRating originalStatus uploadStatus
      genres summary { text }
    }
  }
}
"#;

// The site has its own recently-added feed. Browse sorted by creation date is a
// different set, which is why this section never matched other clients.
const RECENTLY_ADDED_QUERY: &str = r#"
query get_comic_recentlyAdded($select: Comic_RecentlyAdded_Select) {
  get_comic_recentlyAdded(select: $select) {
    items {
      data {
        id name urlPath urlCover translatedLanguage
        type contentRating genres
      }
    }
  }
}
"#;

const LATEST_UPLOADS_QUERY: &str = r#"
query get_comic_latestUploads($select: Comic_LatestUploads_Select) {
  get_comic_latestUploads(select: $select) {
    before
    items {
      comic {
        data {
          id name urlPath urlCover translatedLanguage
          type contentRating genres
        }
      }
      chapters(amount: 1) {
        data { id serial chaNum dname datePublic dateCreate dateModify }
      }
    }
  }
}
"#;

const COMIC_QUERY: &str = r#"
query get_comicNode($id: ID!) {
  get_comicNode(id: $id) {
    data {
      id name type demographics contentRating genres tags
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
    paging { pages }
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
    paging { pages }
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
	pub upload_status: String,
	pub chapter_count: String,
	pub year_min: Option<i64>,
	pub year_max: Option<i64>,
}

impl BrowseParams {
	pub fn new(sortby: &str, page: i32) -> Result<Self> {
		Ok(Self {
			page,
			size: PAGE_SIZE,
			sortby: sortby.into(),
			include_mode: "and".into(),
			exclude_mode: "or".into(),
			excluded_genres: settings::get_excluded_genres(),
			types: settings::get_content_types(),
			content_ratings: settings::get_content_ratings(),
			translated_languages: settings::get_languages()?,
			..Default::default()
		})
	}

	fn allows_pornographic(&self) -> bool {
		self.content_ratings
			.iter()
			.any(|value| value == "pornographic")
	}

	/// A selection covering every option constrains nothing, and sending it anyway
	/// would drop everything the site has not classified yet — which is most of
	/// what a recently-added listing is.
	fn narrowing<'a>(selected: &'a [String], all: &[&str]) -> &'a [String] {
		if selected.len() >= all.len() {
			&[]
		} else {
			selected
		}
	}

	fn select(&self) -> serde_json::Value {
		let mut excluded_genres: Vec<&str> =
			self.excluded_genres.iter().map(String::as_str).collect();
		if !self.allows_pornographic() {
			for genre in PORNOGRAPHIC_GENRES {
				if !excluded_genres.contains(genre) {
					excluded_genres.push(genre);
				}
			}
		}
		serde_json::json!({
			"where": "browse",
			"page": self.page,
			"size": self.size,
			"init": (self.page - 1) * self.size,
			"sortby": self.sortby,
			"word": self.word,
			"incOLangs": self.original_languages,
			"incTLangs": self.translated_languages,
			"incGenres": self.included_genres,
			"excGenres": excluded_genres,
			"incGenresMode": self.include_mode,
			"excGenresMode": self.exclude_mode,
			"incTypes": Self::narrowing(&self.types, settings::ALL_TYPES),
			"incDemographics": self.demographics,
			"incContentRatings": Self::narrowing(&self.content_ratings, settings::ALL_RATINGS),
			"releaseYearMin": self.year_min,
			"releaseYearMax": self.year_max,
			"origStatus": (!self.original_status.is_empty()).then_some(&self.original_status),
			"siteStatus": (!self.upload_status.is_empty()).then_some(&self.upload_status),
			"chapCount": (!self.chapter_count.is_empty()).then_some(&self.chapter_count)
			// The ignoreGlobal* flags stay off. Turning off the site's own blocklist
			// let unapproved uploads through, which lands hardest on the newest ones.
		})
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

	pub fn can_use_latest_uploads(&self) -> bool {
		self.sortby == "field_update"
			&& self.word.is_empty()
			&& self.included_genres.is_empty()
			&& self.demographics.is_empty()
			&& self.original_languages.is_empty()
			&& self.original_status.is_empty()
			&& self.upload_status.is_empty()
			&& self.chapter_count.is_empty()
			&& self.year_min.is_none()
			&& self.year_max.is_none()
	}
}

pub fn graphql_request(
	base_url: &str,
	query: &str,
	variables: serde_json::Value,
) -> Result<Request> {
	let languages = settings::get_languages()?;
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
		.into_iter()
		.map(|node| node.data)
		.filter(|comic| params.allows(comic))
		.collect())
}

pub fn latest_uploads_request(base_url: &str, before: Option<i64>) -> Result<Request> {
	graphql_request(
		base_url,
		LATEST_UPLOADS_QUERY,
		serde_json::json!({
			"select": {
				"size": PAGE_SIZE,
				"before": before
			}
		}),
	)
}

pub fn parse_latest_uploads(
	response: Response,
	params: &BrowseParams,
) -> Result<(Vec<LatestEntry>, Option<i64>)> {
	let response: LatestUploadsResponse = parse_graphql(response)?;
	let LatestUploadsResult { before, items } = response.latest_uploads.unwrap_or_default();
	let mut seen: Vec<String> = Vec::new();
	let comics = items
		.into_iter()
		.filter_map(|item| {
			let comic = item.comic.and_then(|node| node.data)?;
			let chapter = item
				.chapters
				.and_then(|chapters| chapters.into_iter().next())
				.map(|node| node.data);
			Some((comic, chapter))
		})
		.filter(|(comic, _)| params.allows(comic))
		// The feed lists one entry per upload, so a comic repeats per new chapter.
		.filter(|(comic, _)| {
			let unseen = !seen.contains(&comic.id);
			if unseen {
				seen.push(comic.id.clone());
			}
			unseen
		})
		.collect();
	Ok((comics, before))
}

pub fn parse_browse(response: Response, params: &BrowseParams) -> Result<(Vec<ComicData>, bool)> {
	let response: BrowseResponse = parse_graphql(response)?;
	let items = response.get_comic_browse_items;
	let has_next_page = items.len() as i32 >= params.size;
	let comics = items.into_iter().map(|node| node.data).collect();
	Ok((comics, has_next_page))
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
	let pages = first
		.chapter_list
		.as_ref()
		.and_then(|result| result.paging.as_ref())
		.and_then(|paging| paging.pages)
		.unwrap_or(1);
	let mut chapters: Vec<ChapterData> = first
		.chapter_list
		.map(|result| result.items.into_iter().map(|node| node.data).collect())
		.unwrap_or_default();
	for page in 2..=pages {
		let response: ChapterListResponse = graphql(
			base_url,
			query,
			chapter_variables(comic_id, page as i32, size),
		)?;
		if let Some(result) = response.chapter_list {
			chapters.extend(result.items.into_iter().map(|node| node.data));
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
		.map(|data| data.image_urls)
		.unwrap_or_default())
}
