use aidoku::{
	Result,
	alloc::{String, Vec},
	imports::{defaults::defaults_get_map, net::Request},
	prelude::*,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{API_URL, AUTH_KEY, BASE_URL, Senkuro, USER_AGENT};

#[derive(Serialize)]
pub(crate) struct GraphqlEnvelope<'a, V: Serialize> {
	pub(crate) query: &'a str,
	pub(crate) variables: V,
}

#[derive(Deserialize)]
pub(crate) struct GraphqlError {
	pub(crate) message: String,
}

#[derive(Deserialize)]
pub(crate) struct GraphqlResponse<T> {
	pub(crate) data: Option<T>,
	pub(crate) errors: Option<Vec<GraphqlError>>,
}

#[derive(Serialize)]
pub(crate) struct SearchVariables<'a> {
	pub(crate) query: &'a str,
	#[serde(rename = "type")]
	pub(crate) search_type: &'a str,
}

#[derive(Serialize)]
pub(crate) struct SlugVariables<'a> {
	pub(crate) slug: &'a str,
}

#[derive(Serialize)]
pub(crate) struct OrderBy<'a> {
	pub(crate) field: &'a str,
	pub(crate) direction: &'a str,
}

#[derive(Serialize)]
pub(crate) struct ChaptersVariables<'a> {
	pub(crate) branch_id: &'a str,
	pub(crate) number: Option<f32>,
	pub(crate) after: Option<&'a str>,
	pub(crate) order_by: OrderBy<'a>,
}

#[derive(Serialize)]
pub(crate) struct ReaderVariables<'a> {
	pub(crate) slug: &'a str,
	pub(crate) cdn_quality: &'a str,
}

#[derive(Serialize)]
pub(crate) struct HomeVariables<'a> {
	pub(crate) after: Option<&'a str>,
}

pub(crate) const SEARCH_QUERY: &str = r#"query Search($query: String!, $type: SearchType!) {
  search(query: $query, type: $type, first: 10) {
    edges { node {
      ... on SearchManga {
        id slug original_name: originalName
        titles { lang content }
        manga_status: status manga_rating: rating
        cover { blurhash original { url } }
      }
    }}
  }
}"#;

pub(crate) const MANGA_QUERY: &str = r#"query Manga($slug: String!) {
  manga(slug: $slug) {
    id slug original_name: originalName { lang content }
    titles { lang content }
    manga_status: status rating chapters
    mainStaff { person { name } roles }
    branches {
      id primaryBranch
      teamActivities { team { id name slug } }
    }
    cover { blurhash original { height width url } }
    labels { slug titles { lang content } }
  }
}"#;

pub(crate) const HOME_LATEST_QUERY: &str = r#"query HomeLatest($after: String) {
  mangas(
    first: 20 after: $after
    orderBy: { field: LAST_CHAPTER_AT, direction: DESC }
  ) {
    edges { node {
      slug originalName { lang content }
      titles { lang content }
      status rating cover { original { url } }
    }}
    pageInfo { hasNextPage endCursor }
  }
}"#;

pub(crate) const HOME_NEW_QUERY: &str = r#"query HomeNew($after: String) {
  mangas(
    first: 20 after: $after
    orderBy: { field: CREATED_AT, direction: DESC }
  ) {
    edges { node {
      slug originalName { lang content }
      titles { lang content }
      status rating cover { original { url } }
    }}
    pageInfo { hasNextPage endCursor }
  }
}"#;

pub(crate) const HOME_POPULAR_QUERY: &str = r#"query HomePopular {
  mangaPopularByPeriod(period: DAY) {
    slug originalName { lang content }
    titles { lang content }
    status rating cover { original { url } }
  }
}"#;

pub(crate) const HOME_RECOMMENDATIONS_QUERY: &str = r#"query HomeRecommendations {
  mangaRecommendations {
    slug originalName { lang content }
    titles { lang content }
    status rating cover { original { url } }
  }
}"#;

pub(crate) const CHAPTERS_QUERY: &str = r#"query Chapters(
  $branch_id: ID!, $number: Float, $after: String, $order_by: MangaChapterOrder!
) {
  mangaChapters(
    first: 100 branchId: $branch_id number: $number
    after: $after orderBy: $order_by
  ) {
    edges { node {
      id slug team_ids: teamIds name number volume created_at: createdAt
    }}
    pageInfo { hasNextPage endCursor }
  }
}"#;

pub(crate) const READER_QUERY: &str = r#"query Reader($slug: String!, $cdn_quality: String) {
  mangaChapter(slug: $slug) {
    id branch_id: branchId team_ids: teamIds slug
    prev_slug: prevSlug next_slug: nextSlug name number volume
    pages(cdnQuality: $cdn_quality) {
      id number image { original { height width url } }
    }
  }
}"#;

impl Senkuro {
	fn cookies(&self) -> String {
		let Some(cookies) = defaults_get_map(AUTH_KEY) else {
			return String::new();
		};

		let mut header = String::new();
		for (name, value) in cookies.iter() {
			if !header.is_empty() {
				header.push_str("; ");
			}
			header.push_str(name);
			header.push('=');
			header.push_str(value);
		}
		header
	}

	pub(crate) fn graphql<T, V>(&self, query: &str, variables: V) -> Result<T>
	where
		T: DeserializeOwned,
		V: Serialize,
	{
		let body = serde_json::to_string(&GraphqlEnvelope { query, variables })
			.map_err(|_| error!("Senkuro: не удалось собрать GraphQL-запрос"))?;

		let cookie = self.cookies();
		let mut request = Request::post(API_URL)?
			.header("Content-Type", "application/json")
			.header("Accept", "application/json")
			.header("Origin", BASE_URL)
			.header("Referer", BASE_URL)
			.header("User-Agent", USER_AGENT);
		if !cookie.is_empty() {
			request = request.header("Cookie", &cookie);
		}

		let response = request.body(body).send()?;
		if response.status_code() >= 400 {
			return Err(error!("Senkuro API: HTTP ошибка"));
		}

		let envelope = response.get_json_owned::<GraphqlResponse<T>>()?;
		if let Some(error) = envelope.errors.and_then(|mut errors| errors.pop()) {
			bail!("Senkuro API: {}", error.message);
		}

		envelope
			.data
			.ok_or_else(|| error!("Senkuro API: пустой ответ"))
	}
}
