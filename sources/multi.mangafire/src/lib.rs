#![no_std]
use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider,
	Manga, MangaPageResult, MangaStatus, Page, PageContext, Result, Source, Viewer,
	alloc::{String, Vec, string::ToString},
	imports::{html::Html, net::Request, std::send_partial_result},
	prelude::*,
};

mod helpers;
mod home;
mod models;
mod settings;
mod vrf;

use helpers::*;
use models::*;

const BASE_URL: &str = "https://mangafire.to";

struct MangaFire;

impl Source for MangaFire {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut qs = Vec::new();

		// parse filters
		for filter in filters {
			match filter {
				FilterValue::Text { id, value } => match id.as_str() {
					"author" | "artist" => {
						if let Some(tag) = find_tag_id(&value, id.as_str())? {
							qs.push((
								if id == "author" {
									"authors[]"
								} else {
									"artists[]"
								}
								.into(),
								tag.into(),
							));
						}
					}
					"minchap" => qs.push(("min_chap".into(), value.into())),
					_ => bail!("Invalid text filter id"),
				},
				FilterValue::Sort { index, .. } => {
					let (key, value) = match index {
						0 => ("order[relevance]", "desc"),
						1 => ("order[chapter_updated_at]", "desc"),
						2 => ("order[created_at]", "desc"),
						3 => ("order[title]", "asc"),
						4 => ("order[title]", "desc"),
						5 => ("order[year]", "desc"),
						6 => ("order[year]", "asc"),
						7 => ("order[score]", "desc"),
						8 => ("order[views_7d]", "desc"),
						9 => ("order[views_30d]", "desc"),
						10 => ("order[views_total]", "desc"),
						11 => ("order[follows_total]", "desc"),
						_ => bail!("Invalid sort filter index"),
					};
					qs.push((key.into(), value.into()));
				}
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} => match id.as_str() {
					"genres" => {
						for option in included {
							qs.push(("genres_in[]".into(), option.into()));
						}
						for option in excluded {
							qs.push(("genres_ex[]".into(), option.into()));
						}
					}
					_ => {
						for option in included {
							qs.push((id.clone().into(), option.into()));
						}
					}
				},
				FilterValue::Select { id, value } => qs.push((id.into(), value.into())),
				FilterValue::Range { from, to, .. } => {
					if let Some(from) = from {
						qs.push(("year_from".into(), from.to_string().into()));
					}
					if let Some(to) = to {
						qs.push(("year_to".into(), to.to_string().into()));
					}
				}
				_ => {}
			}
		}

		if let Some(query) = query {
			qs.push(("keyword".into(), query.into()));
		}
		qs.push(("page".into(), page.to_string().into()));
		qs.push(("limit".into(), "50".into()));

		api_request("/titles", &mut qs)?
			.header("Accept", "application/json")
			.header("Referer", &format!("{BASE_URL}/"))
			.send()?
			.get_json::<ApiResponse<ApiManga>>()
			.map(|response| MangaPageResult {
				entries: response.items.into_iter().map(Manga::from).collect(),
				has_next_page: response.meta.is_some_and(|meta| meta.has_next),
			})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if manga.key.starts_with("/manga") {
			bail!("Migrate this title to update details.")
		}

		if needs_details {
			let details = api_request(&format!("/titles/{}", manga.key), &mut [])?
				.header("Accept", "application/json")
				.header("Referer", &format!("{BASE_URL}/"))
				.send()?
				.get_json::<ApiDetailsResponse>()?
				.data;

			manga.title = details.title;
			manga.cover = details
				.poster
				.and_then(|poster| poster.large.or(poster.medium).or(poster.small));
			manga.authors = details.authors.map(entity_titles);
			manga.artists = details.artists.map(entity_titles);
			manga.description = details.synopsis_html.and_then(|html| {
				Html::parse_fragment(&html)
					.ok()
					.and_then(|doc| doc.select_first("body").and_then(|body| body.text()))
			});
			manga.url = Some(format!("{BASE_URL}/title/{}", manga.key));
			manga.tags = Some(api_tags(details.genres, details.themes));
			manga.status = match details.status.as_deref() {
				Some("releasing") => MangaStatus::Ongoing,
				Some("finished") => MangaStatus::Completed,
				Some("on_hiatus") => MangaStatus::Hiatus,
				Some("discontinued") => MangaStatus::Cancelled,
				_ => MangaStatus::Unknown,
			};
			manga.content_rating = manga
				.tags
				.as_ref()
				.map(|tags| {
					if tags
						.iter()
						.any(|tag| matches!(tag.as_str(), "Adult" | "Mature" | "Smut"))
					{
						ContentRating::NSFW
					} else if tags.iter().any(|tag| tag == "Ecchi") {
						ContentRating::Suggestive
					} else {
						ContentRating::Unknown
					}
				})
				.unwrap_or_default();
			manga.viewer = match details.manga_type.as_deref() {
				Some("manhua" | "manhwa") => Viewer::Webtoon,
				_ => Viewer::RightToLeft,
			};

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let mut chapters = Vec::new();
			let languages = settings::get_languages()?;
			for lang in &languages {
				let mut page = 1;
				loop {
					let response = api_request(
						&format!("/titles/{}/chapters", manga.key),
						&mut [
							("language".into(), lang.into()),
							("sort".into(), "number".into()),
							("order".into(), "desc".into()),
							("page".into(), page.to_string().into()),
							("limit".into(), "200".into()),
						],
					)?
					.header("Accept", "application/json")
					.header("Referer", &format!("{BASE_URL}/"))
					.send()?
					.get_json::<ApiResponse<ApiChapter>>()?;

					chapters.extend(
						response
							.items
							.into_iter()
							.map(|chapter| chapter.into_chapter(&manga.key, lang)),
					);

					if !response.meta.is_some_and(|meta| meta.has_next) {
						break;
					}
					page += 1;
				}
			}
			if languages.len() > 1 {
				chapters.sort_by_key(|c| core::cmp::Reverse(c.chapter_number.map(|n| n as i32)));
			}

			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		api_request(&format!("/chapters/{}", chapter.key), &mut [])?
			.header("Accept", "application/json")
			.header("Referer", &format!("{BASE_URL}/"))
			.send()?
			.get_json::<ApiPagesResponse>()
			.map(|response| response.data.pages.into_iter().map(Page::from).collect())
	}
}

impl ImageRequestProvider for MangaFire {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", &format!("{BASE_URL}/")))
	}
}

impl DeepLinkHandler for MangaFire {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};

		const TITLE_PATH: &str = "/title/";

		if let Some(path) = path.strip_prefix(TITLE_PATH) {
			// ex: https://mangafire.to/title/dkw-one-piece -> dkw
			// ex: https://mangafire.to/title/pm666-haimiya-senpai-is-scary-but-cute/7511141-chapter-34-en -> pm666
			let key = path
				.split(['-', '/'])
				.next()
				.filter(|hid| !hid.is_empty())
				.map(String::from)
				.ok_or(error!("Missing manga hid"))?;
			Ok(Some(DeepLinkResult::Manga { key }))
		} else {
			Ok(None)
		}
	}
}

register_source!(MangaFire, Home, ImageRequestProvider, DeepLinkHandler);
