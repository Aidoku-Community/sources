use crate::{
	helpers::{editions_of, is_pornographic},
	models::{
		BrowseResponse, ChapterData, ChapterListResponse, ChapterPagesResponse, ComicData,
		ComicNodeResponse, EditionsResponse, GraphQlResponse, LatestUploadsResponse,
		RecentlyAddedResponse, TitleNodeResponse,
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
const CHAPTER_PAGE_SIZE: i64 = 100;
const UNIQUE_CHAPTER_PAGE_SIZE: i64 = 1000;

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
    comicNodes { data { id name subName urlPath translatedLanguage chaps_normal dbStatus isPublic } }
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
    comicNodes { data { id name subName urlPath translatedLanguage chaps_normal dbStatus isPublic } }
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
    comicNodes { data { id name subName urlPath translatedLanguage chaps_normal dbStatus isPublic } }
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
      comicNodes { data { id name subName urlPath translatedLanguage chaps_normal dbStatus isPublic } }
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
      work: title_titleNode { data { authors artists description format_ids } }
    }
  }
}
"#;

const EDITIONS_QUERY: &str = r#"
query get_comicNode($id: ID!) {
  get_comicNode(id: $id) {
    data {
      work: title_titleNode {
        comicNodes { data { id name subName urlPath translatedLanguage chaps_normal dbStatus isPublic } }
      }
    }
  }
}
"#;

const TITLE_QUERY: &str = r#"
query get_title_titleNode($id: ID!) {
  title: get_title_titleNode(id: $id) {
    comicNodes { data { id name subName urlPath translatedLanguage chaps_normal dbStatus isPublic } }
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
        comicNode { data { translatedLanguage } }
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
        comicNode { data { translatedLanguage } }
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
	pub original_status: Vec<String>,
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
			"origStatus": self.original_status,
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
	let mut body = serde_json::json!({ "query": query.trim() });
	body["variables"] = variables;
	Ok(Request::post(format!("{base_url}/query/"))?
		.header("Content-Type", "application/json")
		.header("Accept", "application/json")
		.header("Accept-Language", &accept_language)
		.header("Referer", &format!("{base_url}/"))
		.header("Origin", base_url)
		.body(body.to_string()))
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
	// Merged editions share one chapter list, so one card stands for them all.
	let every = !params.word.is_empty() && !settings::merge_editions();
	let comics = titles
		.into_iter()
		.flat_map(|title| editions_of(title, &params.translated_languages, every))
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

/// A `/title/` page is not itself readable, so it opens the edition browse would show.
pub fn title_edition(base_url: &str, title_id: &str) -> Result<String> {
	let response: TitleNodeResponse =
		graphql(base_url, TITLE_QUERY, serde_json::json!({ "id": title_id }))?;
	let title = response
		.title
		.ok_or_else(|| error!("No comics are available for this series"))?;
	// The reader's languages rank first, but a work with none of them still opens.
	let mut languages = settings::languages();
	languages.extend(
		title
			.comic_nodes
			.iter()
			.flatten()
			.filter_map(|node| node.data.as_ref()?.translated_language.clone()),
	);
	editions_of(title, &languages, false)
		.into_iter()
		.next()
		.map(|comic| comic.id)
		.ok_or_else(|| error!("No comics are available for this series"))
}

/// Every live edition of the work `comic_id` belongs to, in the reader's languages.
pub fn editions(base_url: &str, comic_id: &str) -> Result<Vec<ComicData>> {
	let response: EditionsResponse = graphql(
		base_url,
		EDITIONS_QUERY,
		serde_json::json!({ "id": comic_id }),
	)?;
	let work = response
		.comic
		.and_then(|node| node.data.work)
		.ok_or_else(|| error!("Manga not found"))?;
	let mut editions = editions_of(work, &settings::languages(), true);
	// A shelved edition keeps its chapters after the reader's languages change.
	if !editions.iter().any(|edition| edition.id == comic_id) {
		editions.insert(
			0,
			ComicData {
				id: comic_id.into(),
				..Default::default()
			},
		);
	}
	Ok(editions)
}

fn chapter_page(response: Response) -> Result<(Vec<ChapterData>, Option<i64>, bool)> {
	let response: ChapterListResponse = parse_graphql(response)?;
	let result = response.chapter_list.unwrap_or_default();
	let paging = result.paging.unwrap_or_default();
	Ok((
		result
			.items
			.unwrap_or_default()
			.into_iter()
			.map(|node| node.data)
			.collect(),
		paging.total,
		paging.next.unwrap_or_default() != 0,
	))
}

/// One list per comic, in the order given.
pub fn fetch_chapters(base_url: &str, comic_ids: &[&str]) -> Result<Vec<Vec<ChapterData>>> {
	// The full list carries every scanlator's upload, so it is paged smaller.
	let (query, size) = if settings::deduplicate_chapters() {
		(UNIQUE_CHAPTERS_QUERY, UNIQUE_CHAPTER_PAGE_SIZE)
	} else {
		(CHAPTERS_QUERY, CHAPTER_PAGE_SIZE)
	};
	let request = |comic_id: &str, page: i64| {
		graphql_request(base_url, query, chapter_variables(comic_id, page, size))
	};

	// More than three requests at once and the site answers 429, losing the list.
	let mut lists = Vec::with_capacity(comic_ids.len());
	let mut totals = Vec::with_capacity(comic_ids.len());
	let mut remaining: Vec<(usize, i64)> = Vec::new();
	for batch in comic_ids.chunks(3) {
		let requests = batch
			.iter()
			.map(|comic_id| request(comic_id, 1))
			.collect::<Result<Vec<Request>>>()?;
		for response in Request::send_all(requests) {
			let (chapters, total, has_next) = chapter_page(response?)?;
			let total = total.unwrap_or(chapters.len() as i64);
			if has_next && total > size {
				let index = lists.len();
				remaining.extend((2..=(total + size - 1) / size).map(|page| (index, page)));
			}
			lists.push(chapters);
			totals.push(total);
		}
	}
	for batch in remaining.chunks(3) {
		let requests = batch
			.iter()
			.map(|(index, page)| request(comic_ids[*index], *page))
			.collect::<Result<Vec<Request>>>()?;
		for ((index, _), response) in batch.iter().zip(Request::send_all(requests)) {
			lists[*index].extend(chapter_page(response?)?.0);
		}
	}

	// The total is exact, so a short list lost a page and would otherwise read as complete.
	for (list, total) in lists.iter().zip(&totals) {
		if (list.len() as i64) < *total {
			bail!(
				"Received {} of {total} chapters; refresh to try again",
				list.len()
			);
		}
	}
	Ok(lists)
}

fn chapter_variables(comic_id: &str, page: i64, size: i64) -> serde_json::Value {
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
