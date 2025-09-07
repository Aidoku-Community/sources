use alloc::format;
use alloc::string::{String, ToString};

pub struct GraphQLQuery {
	pub operation_name: &'static str,
	pub query: String,
}

const MANGA_FRAGMENT: &str = r#"fragment MangaFragment on MangaType {
	id
	title
	thumbnailUrl
	author
	artist
	genre
	status
	description
}"#;

const GET_SEARCH_MANGA_LIST_BASE: &str = r#"query GET_SEARCH_MANGA_LIST($condition: MangaConditionInput, $order: [MangaOrderInput!], $filter: MangaFilterInput) {
	mangas(condition: $condition, order: $order, filter: $filter) {
		nodes {
			...MangaFragment
		}
	}
}"#;

const CHAPTER_FRAGMENT: &str = r#"fragment ChapterFragment on ChapterType {
	id
	name
	chapterNumber
	scanlator
	uploadDate
	manga {
	    source {
	        displayName
	    }
	}
}"#;

const GET_MANGA_CHAPTERS_BASE: &str = r#"query GET_MANGA_CHAPTERS($mangaId: Int!) {
	chapters(condition: {mangaId: $mangaId}, order: [{by: SOURCE_ORDER, byType: DESC}]) {
		nodes {
			...ChapterFragment
		}
	}
}"#;

const GET_CHAPTER_PAGES_BASE: &str = r#"mutation GET_PAGE_LIST($input: FetchChapterPagesInput!) {
	fetchChapterPages(input: $input) {
		pages
	}
}"#;

impl GraphQLQuery {
	pub fn get_search_manga_list() -> Self {
		Self {
			operation_name: "GET_SEARCH_MANGA_LIST",
			query: format!("{}\n\n{}", MANGA_FRAGMENT, GET_SEARCH_MANGA_LIST_BASE),
		}
	}

	pub fn get_manga_chapters() -> Self {
		Self {
			operation_name: "GET_MANGA_CHAPTERS",
			query: format!("{}\n\n{}", CHAPTER_FRAGMENT, GET_MANGA_CHAPTERS_BASE),
		}
	}

	pub fn get_chapter_pages() -> Self {
		Self {
			operation_name: "GET_PAGE_LIST",
			query: GET_CHAPTER_PAGES_BASE.to_string(),
		}
	}
}
