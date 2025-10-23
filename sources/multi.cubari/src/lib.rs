#![no_std]
use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Manga, MangaPageResult,
	NotificationHandler, Page, PageContent, Result, Source, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	imports::{net::Request, std::parse_date},
	prelude::*,
};
use core::cell::RefCell;
use core::cmp::Ordering;
use serde_json::Value;

mod database;
mod helpers;
mod remotestorage;
mod settings;
mod urlparser;

use remotestorage::RemoteStorage;
use urlparser::url_to_slug;

#[derive(Default)]
struct Cubari {
	storage: RefCell<Option<RemoteStorage>>,
}

impl Source for Cubari {
	fn new() -> Self {
		Self {
			storage: RefCell::new(RemoteStorage::new()),
		}
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		_page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let slug = query.clone().map(url_to_slug);
		if let Some(slug) = slug
			&& slug.contains('/')
		{
			let manga = self.get_manga_update(
				Manga {
					key: slug,
					..Default::default()
				},
				true,
				false,
			)?;
			Ok(MangaPageResult {
				entries: vec![manga],
				has_next_page: false,
			})
		} else {
			// search existing series
			let query = query.as_deref().unwrap_or_default().to_lowercase();
			let mut entries: Vec<Manga> = if let Some(ref storage) = *self.storage.borrow() {
				storage
					.get_all_series()?
					.into_iter()
					.filter(|manga| manga.title.to_lowercase().contains(&query))
					.collect()
			} else {
				database::series_list()
					.into_iter()
					.filter_map(|key| database::get_manga(key).ok())
					.filter(|manga| manga.title.to_lowercase().contains(&query))
					.collect()
			};
			if settings::get_show_help() {
				entries.insert(0, helpers::guide_manga());
			}
			Ok(MangaPageResult {
				entries,
				has_next_page: false,
			})
		}
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if manga.key == "aidoku/guide" {
			if needs_details || needs_chapters {
				manga.copy_from(helpers::guide_manga());
			}
		} else {
			let json = helpers::get_manga_json(&manga.key)?;

			if needs_details {
				manga.title = json
					.get("title")
					.and_then(|f| f.as_str())
					.map(|s| s.into())
					.ok_or(error!("Missing `title` field"))?;
				manga.cover = json
					.get("cover")
					.and_then(|f| f.as_str())
					.map(|s| helpers::img_url_handler(s.into()));
				manga.artists = json
					.get("artist")
					.and_then(|f| f.as_str())
					.map(|s| vec![s.into()]);
				manga.authors = json
					.get("author")
					.and_then(|f| f.as_str())
					.map(|s| vec![s.into()]);
				manga.description = json
					.get("description")
					.and_then(|f| f.as_str())
					.map(|s| s.into());
				manga.url = Some(format!("https://cubari.moe/read/{}", manga.key));
				manga.content_rating = if manga.key.contains("nhentai") {
					ContentRating::NSFW
				} else {
					ContentRating::Safe
				};
				manga.viewer = Viewer::RightToLeft;

				database::add_or_update_manga(&manga);
			}

			if needs_chapters {
				let chapters_map = json
					.get("chapters")
					.and_then(|f| f.as_object())
					.ok_or(error!("Missing `chapters` field"))?;
				let scanlators_map = json
					.get("groups")
					.and_then(|f| f.as_object())
					.ok_or(error!("Missing `groups` field"))?;
				let mut chapters: Vec<Chapter> = chapters_map
					.iter()
					.filter_map(|(chapter_key, chapter_value)| {
						let groups = chapter_value.get("groups")?.as_object()?;
						Some(
							groups
								.iter()
								.map(|(group, _)| Chapter {
									key: format!("{chapter_key},{group}"),
									title: chapter_value
										.get("title")
										.and_then(|f| f.as_str())
										.map(|s| s.into()),
									chapter_number: chapter_key.parse().ok(),
									volume_number: chapter_value
										.get("volume")
										.and_then(|f| f.as_str())
										.and_then(|s| s.parse().ok()),
									date_uploaded: chapter_value
										.get("release_date")
										.and_then(|f| f.as_object())
										.and_then(|map| map.get(group))
										.and_then(|f| f.as_i64())
										.or_else(|| {
											chapter_value
												.get("date")
												.and_then(|f| f.as_str())
												.and_then(|s| parse_date(s, "yyyy-MM-dd"))
										}),
									scanlators: scanlators_map
										.get(group)
										.and_then(|f| f.as_str())
										.map(|s| vec![s.into()]),
									url: Some(format!(
										"https://cubari.moe/read/{}/{chapter_key}/1",
										manga.key
									)),
									..Default::default()
								})
								.collect::<Vec<_>>(),
						)
					})
					.flatten()
					.collect();
				chapters.sort_by(|a, b| {
					if a.volume_number == b.volume_number {
						if a.chapter_number == b.chapter_number {
							Ordering::Equal
						} else {
							b.chapter_number
								.partial_cmp(&a.chapter_number)
								.unwrap_or(Ordering::Equal)
						}
					} else if a.volume_number.is_none() || b.volume_number.is_none() {
						// chapters with no volume should be on top (reverse order)
						b.volume_number
							.partial_cmp(&a.volume_number)
							.unwrap_or(Ordering::Equal)
					} else {
						b.volume_number
							.partial_cmp(&a.volume_number)
							.unwrap_or(Ordering::Equal)
					}
				});
				manga.chapters = Some(chapters);
			}
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		if manga.key == "aidoku/guide" {
			let content = "Cubari is a proxy for image galleries, and as such there are no manga available by default.

To find a gallery for Cubari, search using the search bar in the format of `<source>/<slug>`, for example, `imgur/hYhqG7b`.

Alternatively, you can paste the link to:
- an imgur/imgbox/catbox/imgchest gallery
- a **raw** GitHub gist link (git.io links may or may not work)
- a manga details page from nhentai, weebcentral, or mangadex
- a cubari.moe reader page

This source locally tracks and saves any series found, which can be disabled in settings.
";
			Ok(vec![Page {
				content: PageContent::text(content),
				..Default::default()
			}])
		} else {
			let mut split = chapter.key.splitn(2, ',');
			let chapter_id = split.next().unwrap_or_default();
			let group = split.next().unwrap_or_default();

			let json = helpers::get_manga_json(&manga.key)?;

			let chapters_map = json
				.get("chapters")
				.and_then(|f| f.as_object())
				.ok_or(error!("Missing `chapters` field"))?;
			let chapter_map = chapters_map
				.get(chapter_id)
				.and_then(|f| f.as_object())
				.ok_or(error!("Missing `{chapter_id}` chapter field"))?;
			let groups = chapter_map
				.get("groups")
				.and_then(|f| f.as_object())
				.ok_or(error!("Missing `groups` field"))?;
			let pages = groups
				.get(group)
				.ok_or(error!("Missing `{group}` group field"))?;

			fn parse_page_array(pages: &[Value]) -> Vec<Page> {
				pages
					.iter()
					.filter_map(|value| {
						let url = if let Some(url) = value.as_str() {
							url.to_string()
						} else if let Some(obj) = value.as_object() {
							obj.get("src")?.as_str()?.into()
						} else {
							return None;
						};
						Some(Page {
							content: PageContent::url(helpers::img_url_handler(url)),
							..Default::default()
						})
					})
					.collect()
			}

			if let Some(array) = pages.as_array() {
				Ok(parse_page_array(array))
			} else if let Some(endpoint) = pages.as_str() {
				let value: Value =
					Request::get(format!("https://cubari.moe{endpoint}"))?.json_owned()?;
				value
					.as_array()
					.map(|array| parse_page_array(array))
					.ok_or(error!("Invalid result from endpoint {endpoint}"))
			} else {
				bail!("Invalid `pages` type")
			}
		}
	}
}

impl DeepLinkHandler for Cubari {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let slug = url_to_slug(url.clone());
		let manga = self.get_manga_update(
			Manga {
				key: slug,
				..Default::default()
			},
			true,
			false,
		)?;
		Ok(Some(DeepLinkResult::Manga { key: manga.key }))
	}
}

impl NotificationHandler for Cubari {
	fn handle_notification(&self, notification: String) {
		match notification.as_str() {
			"deleteHistory" => {
				database::delete_all_manga();
				settings::set_history_revision("");
			}
			settings::ADDRESS_KEY => {
				settings::set_token("");
				let address = settings::get_address();
				if !address.is_empty() && address.contains('@') {
					let provider = address.split('@').next_back().unwrap_or_default();
					let webfinger =
						format!("https://{provider}/.well-known/webfinger?resource=acct:{address}");
					let Some(json) = Request::get(webfinger)
						.ok()
						.and_then(|res| res.json_owned::<Value>().ok())
					else {
						return;
					};
					let Some(links) = json
						.get("links")
						.and_then(|v| v.as_array())
						.and_then(|arr| arr.first())
					else {
						return;
					};
					let storage_url = links.get("href").and_then(|v| v.as_str());
					if let Some(storage_url) = storage_url {
						settings::set_storage_url(storage_url);
					}
					let Some(oauth_url) = links
						.get("properties")
						.and_then(|v| v.get("http://tools.ietf.org/html/rfc6749#section-4.2"))
						.and_then(|v| v.as_str())
					else {
						return;
					};
					settings::set_oauth_url(format!(
						"{oauth_url}?redirect_uri=aidoku%3A%2F%2Fcubari-auth&scope=cubari%3Arw&client_id=aidoku&response_type=token"
					));
				}
			}
			"rsAuthComplete" => {
				let callback = settings::get_token();
				let token: String = callback.split('=').next_back().unwrap_or_default().into();
				if !token.is_empty() {
					settings::set_token(&token);
					*self.storage.borrow_mut() = RemoteStorage::new();
				}
			}
			_ => {}
		}
	}
}

register_source!(Cubari, DeepLinkHandler, NotificationHandler);
