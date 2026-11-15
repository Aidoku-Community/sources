use crate::{
	helpers::tag_name,
	models::{API_URL, BASE_URL, GraphQLResponse, Tag, TagsData},
};
use aidoku::{
	Result,
	alloc::{Vec, string::ToString},
	imports::net::{Request, Response},
	prelude::*,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

pub const POPULAR_QUERY: &str = r#"
query($type: VaildPopularTypeEnumType!, $size: Int!, $page: Int, $dateRange: Int, $allowAdult: Boolean, $allowUnknown: Boolean) {
  queryPopular(type: $type, size: $size, dateRange: $dateRange, page: $page, allowAdult: $allowAdult, allowUnknown: $allowUnknown) {
    total
    recommendations { anyCard { _id name englishName thumbnail isAdult description genres status } }
  }
}
"#;

pub const TRENDING_QUERY: &str = r#"
query($pageSearch: queryPageInput!) {
  queryLatestPageStatus(pageSearch: $pageSearch) {
    total
    recommendations { anyCard { _id name englishName thumbnail isAdult } }
  }
}
"#;

pub const RANDOM_QUERY: &str = r#"
query($format: String!, $allowAdult: Boolean, $denyEcchi: Boolean, $slot: Int) {
  queryRandomRecommendation(format: $format, allowAdult: $allowAdult, denyEcchi: $denyEcchi, slot: $slot) {
    _id name englishName thumbnail isAdult
  }
}
"#;

pub const MANGAS_QUERY: &str = r#"
query($search: SearchInput, $limit: Int, $page: Int, $translationType: VaildTranslationTypeMangaEnumType, $countryOrigin: VaildCountryOriginEnumType) {
  mangas(search: $search, limit: $limit, page: $page, translationType: $translationType, countryOrigin: $countryOrigin) {
    pageInfo { total }
    edges { _id name englishName thumbnail isAdult lastChapterInfo lastChapterDate }
  }
}
"#;

pub const ADAPTATIONS_QUERY: &str = r#"
query($page: Int, $limit: Int, $translationType: VaildTranslationTypeMangaEnumType, $allowAdult: Boolean, $allowUnknown: Boolean, $denyEcchi: Boolean) {
  seasonAdaptationMangas(page: $page, limit: $limit, translationType: $translationType, allowAdult: $allowAdult, allowUnknown: $allowUnknown, denyEcchi: $denyEcchi) {
    pageInfo { total }
    edges { _id name englishName thumbnail isAdult }
  }
}
"#;

pub const CARD_QUERY: &str = r#"
query($id: String!) {
  manga(_id: $id) { _id name englishName thumbnail isAdult }
}
"#;

pub const DETAILS_QUERY: &str = r#"
query($id: String!) {
  manga(_id: $id) {
    _id name englishName thumbnail isAdult description authors genres tags status type countryOfOrigin
  }
}
"#;

pub const CHAPTERS_QUERY: &str = r#"
query($id: String!, $showId: String!) {
  manga(_id: $id) { _id availableChaptersDetail }
  episodeInfos(showId: $showId, episodeNumStart: 0, episodeNumEnd: 9999) { episodeIdNum notes uploadDates }
}
"#;

pub const TAGS_QUERY: &str = r#"
query($search: TagSearchInput, $limit: Int) {
  queryTags(search: $search, limit: $limit, page: 1) { edges { name slug } }
}
"#;

pub fn request(query: &str, variables: Value) -> Result<Request> {
	let mut body = json!({ "query": query.trim() });
	body["variables"] = variables;
	Ok(Request::post(API_URL)?
		.header("Content-Type", "application/json")
		.header("Accept", "application/json")
		.header("Referer", &format!("{BASE_URL}/"))
		.header("Origin", BASE_URL)
		.body(body.to_string()))
}

// The API answers a rejected query with a 400 that still carries GraphQL errors
pub fn parse<T: DeserializeOwned>(response: Response) -> Result<T> {
	let parsed: GraphQLResponse<T> = response.get_json_owned()?;
	if let Some(errors) = parsed.errors.filter(|errors| !errors.is_empty()) {
		let message = errors
			.iter()
			.map(|error| error.message.as_str())
			.collect::<Vec<_>>()
			.join("\n");
		bail!("MKissa: {message}");
	}
	parsed.data.ok_or(error!("MKissa returned no data"))
}

pub fn fetch<T: DeserializeOwned>(query: &str, variables: Value) -> Result<T> {
	parse(request(query, variables)?.send()?)
}

// Details show tag slugs as names, so a tapped name is matched back to its tag
pub fn find_tag(name: &str) -> Result<Option<Tag>> {
	let data: TagsData = fetch(
		TAGS_QUERY,
		json!({ "search": { "query": name, "format": "manga" }, "limit": 5 }),
	)?;
	Ok(data
		.query_tags
		.edges
		.into_iter()
		.find(|tag| tag_name(&tag.slug).eq_ignore_ascii_case(name)))
}
