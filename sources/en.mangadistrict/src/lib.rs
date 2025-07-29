#![no_std]
use aidoku::{
	AidokuError, Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Home,
	HomeComponent, HomeLayout, Listing, ListingProvider, Manga, MangaPageResult, MangaStatus,
	MangaWithChapter, Page, Result, Source, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	imports::{html::Element, net::Request},
	prelude::*,
};

mod filters;
mod helpers;

const BASE_URL: &str = "https://mangadistrict.com";

struct MangaDistrict;

impl Source for MangaDistrict {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let url = format!(
			"{BASE_URL}/page/{page}/?post_type=wp-manga&{}",
			filters::parse_filters(query.clone(), filters)
		);
		let (html, _) = helpers::fetch_html(&url)?;
		let entries = html
			.select(".page-listing-item")
			.map(|items| {
				items
					.filter_map(|item| {
						let url = item.select_first("a")?.attr("href");
						let title = item.select_first(".post-title")?.text()?;
						let cover = item
							.select_first("img")
							.and_then(|img| img.attr("abs:src").or_else(|| img.attr("data-cfsrc")));
						Some(Manga {
							key: url.clone()?.strip_prefix(BASE_URL)?.into(),
							title,
							cover,
							url,
							..Default::default()
						})
					})
					.collect::<Vec<Manga>>()
			})
			.unwrap_or_default();

		Ok(MangaPageResult {
			entries,
			has_next_page: html.select_first(".wp-pagenavi .larger").is_some(),
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let manga_url = format!("{BASE_URL}{}", manga.key);

		if needs_details {
			let (html, now_str) = helpers::fetch_html(&manga_url)?;
			manga.title = html
				.select_first("h1")
				.and_then(|el| el.text())
				.unwrap_or(manga.title);
			manga.cover = html
				.select_first(".summary_image img")
				.and_then(|img| img.attr("abs:src").or_else(|| img.attr("data-cfsrc")));
			manga.artists = html
				.select(".artist-content a")
				.map(|els| els.filter_map(|el| el.text()).collect());
			manga.authors = html
				.select(".author-content a")
				.map(|els| els.filter_map(|el| el.text()).collect());

			// let mut combined: Vec<String> = Vec::new();
			// for name in authors
			// 	.into_iter()
			// 	.chain(manga.artists.clone().into_iter())
			// 	.flatten()
			// {
			// 	if !combined.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
			// 		combined.push(name);
			// 	}
			// }
			// manga.authors = Some(combined);

			manga.description = html
				.select_first(".summary__content > p:nth-child(2)")
				.and_then(|el| el.text())
				.map(|text| text.trim().to_string());
			manga.url = Some(manga_url.clone());
			manga.tags = html
				.select(".tags-content a")
				.map(|els| els.filter_map(|el| el.text()).collect());

			manga.status = html
				.select_first(".post-status .post-content_item:nth-child(2) .summary-content")
				.and_then(|el| el.text())
				.map(|status| match status.to_lowercase().as_str() {
					"canceled" => MangaStatus::Cancelled,
					"completed" => MangaStatus::Completed,
					"ongoing" => MangaStatus::Ongoing,
					"on hold" => MangaStatus::Hiatus,
					_ => MangaStatus::Unknown,
				})
				.unwrap_or(MangaStatus::Unknown);

			manga.update_strategy = match manga.status {
				MangaStatus::Completed | MangaStatus::Cancelled => aidoku::UpdateStrategy::Never,
				_ => aidoku::UpdateStrategy::Always,
			};

			manga.content_rating = if html.select_first(".manga-title-badges.adult").is_some() {
				ContentRating::NSFW
			} else {
				ContentRating::Safe
			};

			manga.viewer = Viewer::Webtoon;

			if needs_chapters {
				manga.chapters = html.select(".version-chap .wp-manga-chapter").map(|els| {
					els.filter_map(|el| {
						let url = el.select_first("a").and_then(|a| a.attr("href"))?;
						let key = url.strip_prefix(BASE_URL)?.into();
						let (chapter_number, title) =
							helpers::parse_chapter_title(el.select_first("a")?.text()).ok()?;
						Some(Chapter {
							key,
							title,
							url: Some(url),
							chapter_number,
							date_uploaded: el
								.select_first(".timediff i")
								.and_then(|i| i.text())
								.or_else(|| {
									el.select_first(".timediff a").and_then(|a| a.attr("title"))
								})
								.and_then(|s| {
									helpers::parse_date_to_timestamp(&s, now_str.as_deref())
								}),
							..Default::default()
						})
					})
					.collect::<Vec<_>>()
				})
			}
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{BASE_URL}{}", chapter.key);
		let html = Request::get(url)?.html()?;
		let pages = html
			.select(".reading-content .page-break img")
			.map(|imgs| {
				imgs.filter_map(|img| {
					let page_url = img.attr("abs:src")?;
					Some(Page {
						content: aidoku::PageContent::Url(page_url, None),
						..Default::default()
					})
				})
				.collect::<Vec<_>>()
			})
			.unwrap_or_default();

		Ok(pages)
	}
}

impl ListingProvider for MangaDistrict {
	fn get_manga_list(&self, _listing: Listing, _page: i32) -> Result<MangaPageResult> {
		Err(AidokuError::Unimplemented)
	}
}

impl Home for MangaDistrict {
	fn get_home(&self) -> Result<HomeLayout> {
		let (html, now_str) = helpers::fetch_html(BASE_URL)?;
		let parse_manga = |el: &Element| -> Option<Manga> {
			let manga_link = el
				.select_first(".post-title a")
				.or_else(|| el.select_first(".widget-title a"))?;
			let manga = Manga {
				key: manga_link.attr("href")?.strip_prefix(BASE_URL)?.into(),
				title: manga_link.text()?,
				cover: el
					.select_first("img")
					.and_then(|img| img.attr("abs:src").or_else(|| img.attr("data-cfsrc"))),
				url: manga_link.attr("href"),
				..Default::default()
			};

			Some(manga)
		};
		let parse_manga_with_chapter = |el: &Element| -> Option<MangaWithChapter> {
			let manga = parse_manga(el)?;
			let chapter_link = el.select_first(".chapter-item a")?;
			let (chapter_number, title) = helpers::parse_chapter_title(chapter_link.text()).ok()?;

			let manga_with_chapter = MangaWithChapter {
				manga,
				chapter: Chapter {
					key: chapter_link.attr("href")?.strip_prefix(BASE_URL)?.into(),
					title,
					chapter_number,
					date_uploaded: el
						.select_first(".timediff a")
						.and_then(|el| el.attr("title"))
						.and_then(|s| helpers::parse_date_to_timestamp(&s, now_str.as_deref())),
					url: chapter_link.attr("href"),
					..Default::default()
				},
			};

			Some(manga_with_chapter)
		};

		let new_releases = html
			.select(".slider__item")
			.map(|els| els.filter_map(|el| parse_manga(&el)).collect::<Vec<_>>())
			.unwrap_or_default();

		let last_updates = html
			.select(".main-col .page-listing-item")
			.map(|els| {
				els.filter_map(|el| parse_manga_with_chapter(&el))
					.collect::<Vec<_>>()
			})
			.unwrap_or_default();

		let todays_trends = html
			.select(".widget-manga-recent .popular-item-wrap")
			.map(|els| els.filter_map(|el| parse_manga(&el)).collect::<Vec<_>>())
			.unwrap_or_default();

		Ok(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some("New Releases".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::Scroller {
						entries: new_releases.clone().into_iter().map(|m| m.into()).collect(),
						listing: None,
					},
				},
				HomeComponent {
					title: Some("Last Updates".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::MangaChapterList {
						page_size: Some(4),
						entries: last_updates,
						listing: None,
					},
				},
				HomeComponent {
					title: Some("Today's Trends".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::Scroller {
						entries: todays_trends.into_iter().map(|m| m.into()).collect(),
						listing: None,
					},
				},
			],
		})
	}
}

impl DeepLinkHandler for MangaDistrict {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		if !url.starts_with(BASE_URL) {
			return Ok(None);
		}

		let key = url.strip_prefix(BASE_URL).unwrap_or(&url).to_string();

		if key.contains("/chapter-") {
			let parts: Vec<&str> = key.split('/').collect();
			if parts.len() < 3 {
				return Ok(None);
			}
			let manga_key = format!("/{}/{}", parts[1], parts[2]);
			return Ok(Some(DeepLinkResult::Chapter { manga_key, key }));
		} else if key.starts_with("/title") {
			return Ok(Some(DeepLinkResult::Manga { key }));
		} else {
			Ok(None)
		}
	}
}

register_source!(MangaDistrict, ListingProvider, Home, DeepLinkHandler);

#[cfg(test)]
mod test {
	use crate::{BASE_URL, MangaDistrict};
	use aidoku::{DeepLinkHandler, DeepLinkResult, Home, Source, alloc::vec, prelude::format};
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn get_home_test() {
		let source = MangaDistrict;
		let home = source.get_home().expect("get_home failed");

		assert_eq!(home.components.len(), 3);
		match &home.components[1].value {
			aidoku::HomeComponentValue::MangaChapterList { entries, .. } => {
				for entry in entries {
					// manga
					assert!(!entry.manga.title.is_empty());
					assert!(!entry.manga.url.is_none());
					assert!(
						entry
							.manga
							.cover
							.as_deref()
							.map_or(false, |img| img.starts_with("https:"))
					);
					// chapter
					assert!(!entry.chapter.url.is_none());
					assert!(entry.chapter.chapter_number.is_some());
				}
			}
			_ => panic!(),
		}
	}

	#[aidoku_test]
	fn get_manga_update_test() {
		let source = MangaDistrict;
		let home = source.get_home().expect("HomeLayout failed");
		if let aidoku::HomeComponentValue::MangaChapterList { entries, .. } =
			&home.components[1].value
		{
			let entry = entries.first().expect("MangaWithChapter not found");
			let manga = source
				.get_manga_update(entry.manga.clone(), true, true)
				.expect("get_manga_update failed");

			// manga
			assert!(!manga.title.is_empty());
			assert!(!manga.url.is_none());
			assert!(
				manga
					.cover
					.as_deref()
					.map_or(false, |img| img.starts_with("https:"))
			);
			assert!(!manga.artists.is_none());
			assert!(!manga.authors.is_none());
			assert!(!manga.description.is_none());
			assert!(!manga.tags.is_none());
			// chapters
			match manga.chapters {
				Some(chapters) => {
					assert!(!chapters.is_empty());
					for chapter in chapters {
						assert!(!chapter.url.is_none());
						assert!(chapter.chapter_number.is_some());
					}
				}
				None => panic!("Chapters should not be empty"),
			}

			assert_eq!(1, 1);
		} else {
			panic!("Expected MangaChapterList");
		}
	}

	#[aidoku_test]
	fn get_search_manga_list_test() {
		let source = MangaDistrict;

		let results = source
			.get_search_manga_list(Some("jungle".into()), 1, vec![])
			.expect("get_search_manga_list failed");
		assert!(!results.entries.is_empty());
	}

	#[aidoku_test]
	fn deep_link_handler_test() {
		let source = MangaDistrict;

		let manga_url = format!("{}/title/my-manga", BASE_URL);
		let manga_result = source
			.handle_deep_link(manga_url)
			.expect("handle_deep_link failed");
		assert_eq!(
			manga_result.unwrap(),
			DeepLinkResult::Manga {
				key: "/title/my-manga".into()
			}
		);

		let chapter_url = format!("{}/title/my-manga/chapter-1", BASE_URL);
		let chapter_result = source
			.handle_deep_link(chapter_url)
			.expect("handle_deep_link failed");
		assert_eq!(
			chapter_result.unwrap(),
			DeepLinkResult::Chapter {
				manga_key: "/title/my-manga".into(),
				key: "/title/my-manga/chapter-1".into()
			}
		);
	}
}
