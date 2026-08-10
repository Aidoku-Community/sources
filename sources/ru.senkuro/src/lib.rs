#![no_std]

use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent,
	HomeComponentValue, HomeLayout, Link, Listing, ListingKind, ListingProvider, Manga,
	MangaPageResult, MangaStatus, Page, PageContent, Result, Source, WebLoginHandler,
	alloc::{String, Vec},
	imports::{defaults::defaults_get_map, net::Request, std::send_partial_result},
	prelude::*,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

const BASE_URL: &str = "https://senkuro.me";
const API_URL: &str = "https://api.senkuro.me/graphql";
const AUTH_KEY: &str = "senkuro_login";
const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1";

const SEARCH_QUERY: &str = r#"query Search($query: String!, $type: SearchType!) {
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

const MANGA_QUERY: &str = r#"query Manga($slug: String!) {
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

const HOME_LATEST_QUERY: &str = r#"query HomeLatest($after: String) {
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

const HOME_NEW_QUERY: &str = r#"query HomeNew($after: String) {
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

const HOME_POPULAR_QUERY: &str = r#"query HomePopular {
  mangaPopularByPeriod(period: DAY) {
    slug originalName { lang content }
    titles { lang content }
    status rating cover { original { url } }
  }
}"#;

const HOME_RECOMMENDATIONS_QUERY: &str = r#"query HomeRecommendations {
  mangaRecommendations {
    slug originalName { lang content }
    titles { lang content }
    status rating cover { original { url } }
  }
}"#;

const CHAPTERS_QUERY: &str = r#"query Chapters(
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

const READER_QUERY: &str = r#"query Reader($slug: String!, $cdn_quality: String) {
  mangaChapter(slug: $slug) {
    id branch_id: branchId team_ids: teamIds slug
    prev_slug: prevSlug next_slug: nextSlug name number volume
    pages(cdnQuality: $cdn_quality) {
      id number image { original { height width url } }
    }
  }
}"#;

#[derive(Serialize)]
struct GraphqlEnvelope<'a, V: Serialize> {
	query: &'a str,
	variables: V,
}

#[derive(Deserialize)]
struct GraphqlError {
	message: String,
}

#[derive(Deserialize)]
struct GraphqlResponse<T> {
	data: Option<T>,
	errors: Option<Vec<GraphqlError>>,
}

#[derive(Serialize)]
struct SearchVariables<'a> {
	query: &'a str,
	#[serde(rename = "type")]
	search_type: &'a str,
}

#[derive(Serialize)]
struct SlugVariables<'a> {
	slug: &'a str,
}

#[derive(Serialize)]
struct OrderBy<'a> {
	field: &'a str,
	direction: &'a str,
}

#[derive(Serialize)]
struct ChaptersVariables<'a> {
	branch_id: &'a str,
	number: Option<f32>,
	after: Option<&'a str>,
	order_by: OrderBy<'a>,
}

#[derive(Serialize)]
struct ReaderVariables<'a> {
	slug: &'a str,
	cdn_quality: &'a str,
}

#[derive(Serialize)]
struct HomeVariables<'a> {
	after: Option<&'a str>,
}

#[derive(Deserialize)]
struct Localized {
	lang: String,
	content: String,
}

#[derive(Deserialize)]
struct Cover {
	original: Option<ImageSize>,
}

#[derive(Deserialize)]
struct ImageSize {
	url: String,
}

#[derive(Deserialize)]
struct SearchData {
	search: SearchConnection<SearchManga>,
}

#[derive(Deserialize)]
struct MangaData {
	manga: MangaInfo,
}

#[derive(Deserialize)]
struct HomeMangasData {
	mangas: HomeMangaConnection,
}

#[derive(Deserialize)]
struct HomePopularData {
	#[serde(rename = "mangaPopularByPeriod")]
	manga_popular_by_period: Vec<HomeManga>,
}

#[derive(Deserialize)]
struct HomeRecommendationsData {
	#[serde(rename = "mangaRecommendations")]
	manga_recommendations: Vec<HomeManga>,
}

#[derive(Deserialize)]
struct ChaptersData {
	#[serde(rename = "mangaChapters")]
	manga_chapters: ChapterConnection,
}

#[derive(Deserialize)]
struct ReaderData {
	#[serde(rename = "mangaChapter")]
	manga_chapter: ReaderChapter,
}

#[derive(Deserialize)]
struct SearchConnection<T> {
	edges: Vec<Edge<T>>,
}

#[derive(Deserialize)]
struct HomeMangaConnection {
	edges: Vec<Edge<HomeManga>>,
	#[serde(rename = "pageInfo")]
	page_info: PageInfo,
}

#[derive(Deserialize)]
struct ChapterConnection {
	edges: Vec<Edge<RemoteChapter>>,
	#[serde(rename = "pageInfo")]
	page_info: PageInfo,
}

#[derive(Deserialize)]
struct PageInfo {
	#[serde(rename = "hasNextPage")]
	has_next_page: bool,
	#[serde(rename = "endCursor")]
	end_cursor: Option<String>,
}

#[derive(Deserialize)]
struct Edge<T> {
	node: T,
}

#[derive(Deserialize)]
struct SearchManga {
	slug: String,
	original_name: String,
	titles: Vec<Localized>,
	manga_status: String,
	manga_rating: String,
	cover: Option<Cover>,
}

#[derive(Deserialize)]
struct HomeManga {
	slug: String,
	#[serde(rename = "originalName")]
	original_name: Localized,
	titles: Vec<Localized>,
	status: String,
	rating: String,
	cover: Option<Cover>,
}

#[derive(Deserialize)]
struct Branch {
	id: String,
	#[serde(rename = "primaryBranch")]
	primary_branch: bool,
	#[serde(rename = "teamActivities")]
	team_activities: Vec<TeamActivity>,
}

#[derive(Deserialize)]
struct TeamActivity {
	team: Team,
}

#[derive(Deserialize)]
struct Team {
	id: String,
	name: String,
	#[allow(dead_code)]
	slug: String,
}

#[derive(Deserialize)]
struct Label {
	slug: String,
	titles: Vec<Localized>,
}

#[derive(Deserialize)]
struct MangaInfo {
	slug: String,
	original_name: Localized,
	titles: Vec<Localized>,
	manga_status: String,
	rating: String,
	#[serde(rename = "mainStaff")]
	main_staff: Vec<StaffMember>,
	branches: Vec<Branch>,
	cover: Option<Cover>,
	labels: Vec<Label>,
}

#[derive(Deserialize)]
struct StaffMember {
	person: Person,
	roles: Vec<String>,
}

#[derive(Deserialize)]
struct Person {
	name: String,
}

#[derive(Deserialize)]
struct RemoteChapter {
	slug: String,
	team_ids: Vec<String>,
	name: Option<String>,
	number: String,
	volume: String,
}

#[derive(Deserialize)]
struct ReaderChapter {
	pages: Vec<ReaderPage>,
}

#[derive(Deserialize)]
struct ReaderPage {
	image: Option<Cover>,
}

struct Senkuro;

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

	fn graphql<T, V>(&self, query: &str, variables: V) -> Result<T>
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
			return Err(error!("Senkuro API: {}", error.message));
		}

		envelope
			.data
			.ok_or_else(|| error!("Senkuro API: пустой ответ"))
	}

	fn title(localized: &Localized, titles: &[Localized]) -> String {
		for item in titles {
			if item.lang == "RU" {
				return item.content.clone();
			}
		}
		if !localized.content.is_empty() {
			localized.content.clone()
		} else {
			titles
				.first()
				.map(|item| item.content.clone())
				.unwrap_or_default()
		}
	}

	fn title_string(original: &str, titles: &[Localized]) -> String {
		for item in titles {
			if item.lang == "RU" {
				return item.content.clone();
			}
		}
		if !original.is_empty() {
			String::from(original)
		} else {
			titles
				.first()
				.map(|item| item.content.clone())
				.unwrap_or_default()
		}
	}

	fn rating(value: &str) -> ContentRating {
		match value {
			"EXPLICIT" => ContentRating::NSFW,
			"QUESTIONABLE" | "SENSITIVE" => ContentRating::Suggestive,
			"GENERAL" => ContentRating::Safe,
			_ => ContentRating::Unknown,
		}
	}

	fn status(value: &str) -> MangaStatus {
		match value {
			"ONGOING" => MangaStatus::Ongoing,
			"FINISHED" | "RELEASED" => MangaStatus::Completed,
			"SUSPENDED" => MangaStatus::Hiatus,
			"CANCELLED" => MangaStatus::Cancelled,
			_ => MangaStatus::Unknown,
		}
	}

	fn cover(cover: Option<&Cover>) -> Option<String> {
		cover.and_then(|value| value.original.as_ref().map(|image| image.url.clone()))
	}

	fn staff_names(staff: &[StaffMember], roles: &[&str]) -> Vec<String> {
		staff
			.iter()
			.filter(|member| {
				member
					.roles
					.iter()
					.any(|role| roles.iter().any(|needle| role.contains(needle)))
			})
			.map(|member| member.person.name.clone())
			.collect()
	}

	fn home_manga(item: HomeManga) -> Manga {
		Manga {
			key: item.slug.clone(),
			title: Self::title(&item.original_name, &item.titles),
			cover: Self::cover(item.cover.as_ref()),
			url: Some(format!("{BASE_URL}/manga/{}", item.slug)),
			status: Self::status(&item.status),
			content_rating: Self::rating(&item.rating),
			..Default::default()
		}
	}

	fn home_listing(id: &str, name: &str) -> Listing {
		Listing {
			id: id.into(),
			name: name.into(),
			kind: ListingKind::Default,
		}
	}

	fn home_manga_page(&self, query: &str, page: i32) -> Result<MangaPageResult> {
		if page < 1 {
			return Ok(MangaPageResult {
				entries: Vec::new(),
				has_next_page: false,
			});
		}

		let mut cursor: Option<String> = None;
		let mut current_page = 1;
		loop {
			let data: HomeMangasData = self.graphql(
				query,
				HomeVariables {
					after: cursor.as_deref(),
				},
			)?;
			let connection = data.mangas;
			let has_next_page = connection.page_info.has_next_page;
			let next_cursor = connection.page_info.end_cursor;

			if current_page == page {
				return Ok(MangaPageResult {
					entries: connection
						.edges
						.into_iter()
						.map(|edge| Self::home_manga(edge.node))
						.collect(),
					has_next_page,
				});
			}

			if !has_next_page {
				return Ok(MangaPageResult {
					entries: Vec::new(),
					has_next_page: false,
				});
			}

			cursor = Some(
				next_cursor.ok_or_else(|| error!("Senkuro: Home listing has no next cursor"))?,
			);
			current_page += 1;
		}
	}

	fn home_component(title: &str, id: &str, entries: Vec<Manga>) -> HomeComponent {
		let links = entries.into_iter().map(Link::from).collect();
		HomeComponent {
			title: Some(title.into()),
			subtitle: None,
			value: HomeComponentValue::Scroller {
				entries: links,
				listing: Some(Self::home_listing(id, title)),
			},
		}
	}

	fn home_static_list(&self, query: &str) -> Result<Vec<Manga>> {
		let data: HomePopularData = self.graphql(query, HomeVariables { after: None })?;
		Ok(data
			.manga_popular_by_period
			.into_iter()
			.map(Self::home_manga)
			.collect())
	}

	fn home_recommendations(&self) -> Result<Vec<Manga>> {
		let data: HomeRecommendationsData =
			self.graphql(HOME_RECOMMENDATIONS_QUERY, HomeVariables { after: None })?;
		Ok(data
			.manga_recommendations
			.into_iter()
			.map(Self::home_manga)
			.collect())
	}

	fn search_manga(&self, item: SearchManga) -> Manga {
		let title = Self::title_string(&item.original_name, &item.titles);
		Manga {
			key: item.slug.clone(),
			title,
			cover: Self::cover(item.cover.as_ref()),
			url: Some(format!("{BASE_URL}/manga/{}", item.slug)),
			status: Self::status(&item.manga_status),
			content_rating: Self::rating(&item.manga_rating),
			..Default::default()
		}
	}

	fn fetch_description(&self, slug: &str) -> Option<String> {
		let url = format!("{BASE_URL}/manga/{slug}");
		let mut request = Request::get(url).ok()?;
		request = request.header("User-Agent", USER_AGENT);
		let response = request.send().ok()?;
		response
			.get_html()
			.ok()?
			.select_first("meta[name=description]")
			.and_then(|element| element.attr("content"))
	}

	fn chapter_list(&self, manga: &MangaInfo) -> Result<Vec<Chapter>> {
		let branch = manga
			.branches
			.iter()
			.find(|branch| branch.primary_branch)
			.or_else(|| manga.branches.first())
			.ok_or_else(|| error!("Senkuro: у тайтла нет ветки перевода"))?;

		let mut chapters = Vec::new();
		let mut cursor: Option<String> = None;

		loop {
			let data: ChaptersData = self.graphql(
				CHAPTERS_QUERY,
				ChaptersVariables {
					branch_id: &branch.id,
					number: None,
					after: cursor.as_deref(),
					order_by: OrderBy {
						field: "NUMBER",
						direction: "DESC",
					},
				},
			)?;

			let connection = data.manga_chapters;
			let has_next_page = connection.page_info.has_next_page;
			let next_cursor = connection.page_info.end_cursor.clone();

			chapters.extend(connection.edges.into_iter().map(|edge| {
				let chapter = edge.node;
				let scanlators = branch
					.team_activities
					.iter()
					.filter(|activity| chapter.team_ids.contains(&activity.team.id))
					.map(|activity| activity.team.name.clone())
					.collect::<Vec<_>>();

				Chapter {
					key: chapter.slug.clone(),
					title: chapter.name,
					chapter_number: chapter.number.parse::<f32>().ok(),
					volume_number: chapter.volume.parse::<f32>().ok(),
					scanlators: if scanlators.is_empty() {
						None
					} else {
						Some(scanlators)
					},
					url: Some(format!("{BASE_URL}/manga/{}/chapters", manga.slug)),
					locked: false,
					..Default::default()
				}
			}));

			if !has_next_page {
				break;
			}

			let next_cursor = next_cursor
				.ok_or_else(|| error!("Senkuro: у списка глав нет следующего курсора"))?;
			if cursor.as_deref() == Some(next_cursor.as_str()) {
				return Err(error!("Senkuro: пагинация списка глав зациклилась"));
			}
			cursor = Some(next_cursor);
		}

		Ok(chapters)
	}
}

impl Source for Senkuro {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		_page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let query = query.unwrap_or_default();
		if query.trim().is_empty() {
			return Ok(MangaPageResult {
				entries: Vec::new(),
				has_next_page: false,
			});
		}

		let data: SearchData = self.graphql(
			SEARCH_QUERY,
			SearchVariables {
				query: &query,
				search_type: "MANGA",
			},
		)?;

		Ok(MangaPageResult {
			entries: data
				.search
				.edges
				.into_iter()
				.map(|edge| self.search_manga(edge.node))
				.collect(),
			has_next_page: false,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let data: MangaData = self.graphql(MANGA_QUERY, SlugVariables { slug: &manga.key })?;
		let remote = data.manga;

		if needs_details {
			manga.title = Self::title(&remote.original_name, &remote.titles);
			manga.cover = Self::cover(remote.cover.as_ref());
			manga.url = Some(format!("{BASE_URL}/manga/{}", remote.slug));
			manga.status = Self::status(&remote.manga_status);
			manga.content_rating = Self::rating(&remote.rating);
			let authors =
				Self::staff_names(&remote.main_staff, &["AUTHOR", "WRITER", "STORY", "SCRIPT"]);
			manga.authors = if authors.is_empty() {
				None
			} else {
				Some(authors)
			};
			let artists = Self::staff_names(
				&remote.main_staff,
				&["ART", "ARTIST", "ILLUSTRATOR", "DRAWER"],
			);
			manga.artists = if artists.is_empty() {
				None
			} else {
				Some(artists)
			};
			manga.description = self.fetch_description(&remote.slug);

			let mut tags = Vec::new();
			for label in &remote.labels {
				let label_title = Self::title(
					&Localized {
						lang: String::new(),
						content: label.slug.clone(),
					},
					&label.titles,
				);
				if !label_title.is_empty() {
					tags.push(label_title);
				}
			}
			manga.tags = Some(tags);
		}

		if needs_chapters {
			if needs_details {
				send_partial_result(&manga);
			}
			manga.chapters = Some(self.chapter_list(&remote)?);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let data: ReaderData = self.graphql(
			READER_QUERY,
			ReaderVariables {
				slug: &chapter.key,
				cdn_quality: "red",
			},
		)?;

		Ok(data
			.manga_chapter
			.pages
			.into_iter()
			.filter_map(|page| {
				let url = page.image?.original?.url;
				Some(Page {
					content: PageContent::url(url),
					..Default::default()
				})
			})
			.collect())
	}
}

impl ListingProvider for Senkuro {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		match listing.id.as_str() {
			"latest-updates" => self.home_manga_page(HOME_LATEST_QUERY, page),
			"new-titles" => self.home_manga_page(HOME_NEW_QUERY, page),
			"popular-day" => {
				if page != 1 {
					return Ok(MangaPageResult {
						entries: Vec::new(),
						has_next_page: false,
					});
				}
				Ok(MangaPageResult {
					entries: self.home_static_list(HOME_POPULAR_QUERY)?,
					has_next_page: false,
				})
			}
			"recommendations" => {
				if page != 1 {
					return Ok(MangaPageResult {
						entries: Vec::new(),
						has_next_page: false,
					});
				}
				Ok(MangaPageResult {
					entries: self.home_recommendations()?,
					has_next_page: false,
				})
			}
			_ => Err(error!("Senkuro: неизвестная Home-секция")),
		}
	}
}

impl Home for Senkuro {
	fn get_home(&self) -> Result<HomeLayout> {
		let latest = self.home_manga_page(HOME_LATEST_QUERY, 1)?.entries;
		let popular = self.home_static_list(HOME_POPULAR_QUERY)?;
		let new_titles = self.home_manga_page(HOME_NEW_QUERY, 1)?.entries;

		if latest.is_empty() || popular.is_empty() || new_titles.is_empty() {
			return Err(error!("Senkuro: Home не вернула обязательные секции"));
		}

		let mut components = Vec::new();
		components.push(Self::home_component(
			"Свежие обновления",
			"latest-updates",
			latest,
		));
		components.push(Self::home_component(
			"Популярное за день",
			"popular-day",
			popular,
		));
		components.push(Self::home_component(
			"Новые тайтлы",
			"new-titles",
			new_titles,
		));

		let recommendations = self.home_recommendations()?;
		if !recommendations.is_empty() {
			components.push(Self::home_component(
				"Рекомендации",
				"recommendations",
				recommendations,
			));
		}

		Ok(HomeLayout { components })
	}
}

impl DeepLinkHandler for Senkuro {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url
			.strip_prefix(BASE_URL)
			.or_else(|| url.strip_prefix("https://senkuro.com"))
			.map(|path| path.trim_start_matches('/'))
		else {
			return Ok(None);
		};

		let mut parts = path.split('/').filter(|part| !part.is_empty());
		if parts.next() != Some("manga") {
			return Ok(None);
		}

		let Some(slug) = parts.next() else {
			return Ok(None);
		};

		Ok(Some(DeepLinkResult::Manga { key: slug.into() }))
	}
}

impl WebLoginHandler for Senkuro {
	fn handle_web_login(
		&self,
		key: String,
		cookies: aidoku::HashMap<String, String>,
	) -> Result<bool> {
		if key != AUTH_KEY || cookies.is_empty() {
			return Ok(false);
		}

		let mut cookie_header = String::new();
		for (name, value) in cookies.iter() {
			if !cookie_header.is_empty() {
				cookie_header.push_str("; ");
			}
			cookie_header.push_str(name);
			cookie_header.push('=');
			cookie_header.push_str(value);
		}

		let response = Request::get(format!(
			"{BASE_URL}/manga/i-took-it-instead-of-my-husband/chapters"
		))?
		.header("User-Agent", USER_AGENT)
		.header(
			"Accept",
			"text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
		)
		.header("Referer", BASE_URL)
		.header("Cookie", &cookie_header)
		.send()?;

		if response.status_code() >= 400 {
			return Ok(false);
		}

		let body = response
			.get_html()?
			.select_first("body")
			.and_then(|element| element.text())
			.unwrap_or_default();

		Ok(body.contains("18+") && !body.contains("Авторизуйтесь для чтения"))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn decodes_senkuro_chapter_detail_contract() {
		let payload = r#"{
          "data": {
            "manga": {
              "slug": "demo",
              "original_name": {"lang": "EN", "content": "Demo"},
              "titles": [],
              "manga_status": "ONGOING",
              "rating": "EXPLICIT",
              "mainStaff": [],
              "branches": [{"id": "branch-1", "primaryBranch": true, "teamActivities": []}],
              "cover": null,
              "labels": []
            }
          }
        }"#;

		let parsed: GraphqlResponse<MangaData> = serde_json::from_str(payload).unwrap();
		assert_eq!(parsed.data.unwrap().manga.branches[0].id, "branch-1");
	}
}

register_source!(
	Senkuro,
	WebLoginHandler,
	ListingProvider,
	Home,
	DeepLinkHandler
);
