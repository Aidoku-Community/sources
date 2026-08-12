#![no_std]

mod graphql;
mod helpers;
mod models;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeLayout, Listing,
	ListingProvider, Manga, MangaPageResult, Page, PageContent, Result, Source, WebLoginHandler,
	alloc::{String, Vec},
	imports::{net::Request, std::send_partial_result},
	prelude::*,
};

use crate::graphql::{
	CHAPTERS_QUERY, ChaptersVariables, HOME_LATEST_QUERY, HOME_NEW_QUERY, HOME_POPULAR_QUERY,
	HomeVariables, MANGA_QUERY, OrderBy, READER_QUERY, ReaderVariables, SEARCH_QUERY,
	SearchVariables, SlugVariables,
};
use crate::models::{
	ChapterConnection, ChaptersData, HomeMangasData, MangaData, MangaInfo, ReaderData, SearchData,
};

const BASE_URL: &str = "https://senkuro.me";
const API_URL: &str = "https://api.senkuro.me/graphql";
const AUTH_KEY: &str = "senkuro_login";
const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1";

struct Senkuro;

impl Senkuro {
	fn home_manga_page(&self, query: &str, page: i32) -> Result<MangaPageResult> {
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

			let ChapterConnection { edges, page_info } = data.manga_chapters;
			let has_next_page = page_info.has_next_page;
			let next_cursor = page_info.end_cursor;

			chapters.extend(edges.into_iter().map(|edge| {
				let chapter = edge.node;
				let scanlators = branch
					.team_activities
					.iter()
					.filter(|activity| chapter.team_ids.contains(&activity.team.id))
					.map(|activity| activity.team.name.clone())
					.collect::<Vec<_>>();

				Chapter {
					key: chapter.slug,
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
		let Some(query) = query else {
			return Ok(MangaPageResult::default());
		};

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
		let mut remote = data.manga;

		if needs_details {
			manga.title = Self::title(&remote.original_name.content, &remote.titles);
			manga.cover = Self::cover(remote.cover.take());
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
				let label_title = Self::title(&label.slug, &label.titles);
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
			"popular-day" => Ok(MangaPageResult {
				entries: self.home_static_list(HOME_POPULAR_QUERY)?,
				has_next_page: false,
			}),
			"recommendations" => Ok(MangaPageResult {
				entries: self.home_recommendations()?,
				has_next_page: false,
			}),
			_ => bail!("Senkuro: неизвестная Home-секция"),
		}
	}
}

impl Home for Senkuro {
	fn get_home(&self) -> Result<HomeLayout> {
		let latest = self.home_manga_page(HOME_LATEST_QUERY, 1)?.entries;
		let popular = self.home_static_list(HOME_POPULAR_QUERY)?;
		let new_titles = self.home_manga_page(HOME_NEW_QUERY, 1)?.entries;

		if latest.is_empty() || popular.is_empty() || new_titles.is_empty() {
			bail!("Senkuro: Home не вернула обязательные секции");
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
	use crate::graphql::GraphqlResponse;
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
