use alloc::string::{String, ToString};

pub struct GraphQLQuery {
	pub operation_name: &'static str,
	pub query: String,
}

const GET_SEARCH_MANGA_LIST: &str = r#"query GET_SEARCH_MANGA_LIST($condition: MangaConditionInput, $order: [MangaOrderInput!], $filter: MangaFilterInput) {
	mangas(condition: $condition, order: $order, filter: $filter) {
		nodes {
			id
			title
			thumbnailUrl
			author
			artist
			genre
			status
		}
	}
}"#;

const GET_MANGA_CHAPTERS: &str = r#"query GET_MANGA_CHAPTERS($mangaId: Int!) {
	chapters(condition: {mangaId: $mangaId}, order: [{by: SOURCE_ORDER, byType: DESC}]) {
		nodes {
			id
			name
			chapterNumber
			scanlator
			uploadDate
			sourceOrder
			manga {
				source {
					displayName
				}
			}
		}
	}
}"#;

const GET_CHAPTER_PAGES: &str = r#"mutation GET_PAGE_LIST($input: FetchChapterPagesInput!) {
	fetchChapterPages(input: $input) {
		pages
	}
}"#;

const GET_MANGA_DESCRIPTION: &str = r#"query GET_MANGA_DESCRIPTION($mangaId: Int!) {
	manga(id: $mangaId) {
		description
	}
}
"#;

impl GraphQLQuery {
	pub fn get_search_manga_list() -> Self {
		Self {
			operation_name: "GET_SEARCH_MANGA_LIST",
			query: GET_SEARCH_MANGA_LIST.to_string(),
		}
	}

	pub fn get_manga_chapters() -> Self {
		Self {
			operation_name: "GET_MANGA_CHAPTERS",
			query: GET_MANGA_CHAPTERS.to_string(),
		}
	}

	pub fn get_chapter_pages() -> Self {
		Self {
			operation_name: "GET_PAGE_LIST",
			query: GET_CHAPTER_PAGES.to_string(),
		}
	}

	pub fn get_manga_description() -> Self {
		Self {
			operation_name: "GET_MANGA_DESCRIPTION",
			query: GET_MANGA_DESCRIPTION.to_string(),
		}
	}
}
