pub struct GraphQLQuery {
	pub operation_name: &'static str,
	pub query: &'static str,
}

const GET_HQS_BY_NAME: &str = r#"query getHqsByName($name: String!) {
	getHqsByName(name: $name) {
		id
		name
		hqCover
		synopsis
	}
}"#;

const GET_RECENTLY_UPDATED_HQS: &str = r#"query getRecentlyUpdatedHqs {
	getRecentlyUpdatedHqs {
		id
		name
		hqCover
		synopsis
	}
}"#;

const GET_HQS_BY_FILTERS: &str = r#"query getHqsByFilters($publisherId: Int, $orderByViews: Boolean, $limit: Int, $loadCovers: Boolean) {
	getHqsByFilters(publisherId: $publisherId, orderByViews: $orderByViews, limit: $limit, loadCovers: $loadCovers) {
		id
		name
		hqCover
		synopsis
	}
}"#;

const GET_HQS_BY_ID: &str = r#"query getHqsById($id: Int!) {
	getHqsById(id: $id) {
		id
		name
		synopsis
		hqCover
		publisherName
		status
		capitulos {
			id
			name
			number
		}
	}
}"#;

const GET_CHAPTER_BY_ID: &str = r#"query getChapterById($chapterId: Int!) {
	getChapterById(chapterId: $chapterId) {
		pictures {
			pictureUrl
		}
	}
}"#;

const GET_CAROUSEL_OF_HQS: &str = r#"query getCarouselOfHqs {
	getCarouselOfHqs {
		hqId
		name
		hqCover
	}
}"#;

impl GraphQLQuery {
	pub const HQS_BY_NAME: Self = Self {
		operation_name: "getHqsByName",
		query: GET_HQS_BY_NAME,
	};

	pub const RECENTLY_UPDATED: Self = Self {
		operation_name: "getRecentlyUpdatedHqs",
		query: GET_RECENTLY_UPDATED_HQS,
	};

	pub const HQS_BY_FILTERS: Self = Self {
		operation_name: "getHqsByFilters",
		query: GET_HQS_BY_FILTERS,
	};

	pub const HQS_BY_ID: Self = Self {
		operation_name: "getHqsById",
		query: GET_HQS_BY_ID,
	};

	pub const CHAPTER_BY_ID: Self = Self {
		operation_name: "getChapterById",
		query: GET_CHAPTER_BY_ID,
	};

	pub const CAROUSEL: Self = Self {
		operation_name: "getCarouselOfHqs",
		query: GET_CAROUSEL_OF_HQS,
	};
}
