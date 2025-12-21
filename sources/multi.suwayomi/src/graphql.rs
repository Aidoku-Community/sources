pub struct GraphQLQuery {
	pub operation_name: &'static str,
	pub query: &'static str,
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

const GET_CHAPTER_PAGES: &str = r#"mutation GET_CHAPTER_PAGES($input: FetchChapterPagesInput!) {
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

const GET_CATEGORIES: &str = r#"query GET_CATEGORIES {
	categories {
		nodes {
			name
			id
		}
	}
}"#;

impl GraphQLQuery {
	pub const SEARCH_MANGA_LIST: Self = Self {
		operation_name: "GET_SEARCH_MANGA_LIST",
		query: GET_SEARCH_MANGA_LIST,
	};

	pub const MANGA_CHAPTERS: Self = Self {
		operation_name: "GET_MANGA_CHAPTERS",
		query: GET_MANGA_CHAPTERS,
	};

	pub const CHAPTER_PAGES: Self = Self {
		operation_name: "GET_CHAPTER_PAGES",
		query: GET_CHAPTER_PAGES,
	};

	pub const MANGA_DESCRIPTION: Self = Self {
		operation_name: "GET_MANGA_DESCRIPTION",
		query: GET_MANGA_DESCRIPTION,
	};

	pub const CATEGORIES: Self = Self {
		operation_name: "GET_CATEGORIES",
		query: GET_CATEGORIES,
	};
}
