use crate::{MangaFire, BASE_URL};
use aidoku::helpers::string::StripPrefixOrSelf;
use aidoku::imports::html::Document;
use aidoku::{
	alloc::{vec, Vec},
	imports::{net::Request, std::send_partial_result},
	prelude::*,
	Home, HomeComponent, HomeLayout, HomePartialResult, Manga, MangaWithChapter, Result,
};
use aidoku::{Chapter, Link};

impl Home for MangaFire {
	fn get_home(&self) -> Result<HomeLayout> {
		// send basic home layout
		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: vec![
				HomeComponent {
					title: None,
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_big_scroller(),
				},
				HomeComponent {
					title: Some("Most Viewed".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("Recently Updated".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_manga_chapter_list(),
				},
				HomeComponent {
					title: Some("New Release".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_scroller(),
				},
			],
		}));

		let html = Request::get(format!("{BASE_URL}/home"))?.html()?;

		fn parse_scroller_entries(html: &Document, section_selector: &str) -> Vec<Link> {
			html.select_first(section_selector)
				.and_then(|section| {
					section
						.select(".swiper-wrapper > .swiper-slide")
						.map(|els| {
							els.filter_map(|el| {
								Some(
									Manga {
										key: el
											.select_first("a")?
											.attr("href")?
											.strip_prefix_or_self(BASE_URL)
											.into(),
										title: el
											.select_first("a > span")?
											.text()
											.unwrap_or_default(),
										cover: el
											.select_first(".poster img")
											.and_then(|img| img.attr("src")),
										..Default::default()
									}
									.into(),
								)
							})
							.collect()
						})
				})
				.unwrap_or_default()
		}

		let components = vec![
			HomeComponent {
				title: None,
				subtitle: None,
				value: aidoku::HomeComponentValue::BigScroller {
					entries: html
						.select(".trending > .swiper-wrapper > .swiper-slide")
						.map(|els| {
							els.filter_map(|el| {
								let link = el.select_first(".info > .above > a")?;
								let key = link.attr("href")?.strip_prefix_or_self(BASE_URL).into();
								Some(Manga {
									key,
									title: link.text().unwrap_or_default(),
									cover: el.select_first("img").and_then(|img| img.attr("src")),
									description: el
										.select_first(".info > .below > span")
										.and_then(|el| el.text()),
									tags: el
										.select(".info > .below a")
										.map(|els| els.filter_map(|el| el.text()).collect()),
									..Default::default()
								})
							})
							.collect()
						})
						.unwrap_or_default(),
					auto_scroll_interval: Some(10.0),
				},
			},
			HomeComponent {
				title: Some("Most Viewed".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::Scroller {
					entries: parse_scroller_entries(&html, "#most-viewed"),
					listing: None,
				},
			},
			HomeComponent {
				title: Some("Recently Updated".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::MangaChapterList {
					page_size: Some(12),
					entries: html
						.select("section > .tab-content > .original > .unit")
						.map(|els| {
							els.filter_map(|el| {
								let chapter = el.select_first("ul.content > li")?;
								Some(MangaWithChapter {
									manga: Manga {
										key: el
											.select_first("a")?
											.attr("href")?
											.strip_prefix_or_self(BASE_URL)
											.into(),
										title: el
											.select_first(".info a")
											.and_then(|el| el.text())
											.unwrap_or_default(),
										cover: el
											.select_first(".poster img")
											.and_then(|img| img.attr("src")),
										..Default::default()
									},
									chapter: Chapter {
										key: el
											.select_first("a")?
											.attr("href")?
											.strip_prefix_or_self(BASE_URL)
											.into(),
										chapter_number: chapter
											.select_first("a > span")
											.and_then(|el| el.own_text())
											.and_then(|el| {
												el.strip_prefix("Chap ")
													.and_then(|s| s.parse().ok())
											}),
										// date_uploaded: (), // todo: parse relative dates
										..Default::default()
									},
								})
							})
							.collect()
						})
						.unwrap_or_default(),
					listing: None,
				},
			},
			HomeComponent {
				title: Some("New Release".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::Scroller {
					entries: parse_scroller_entries(&html, ".swiper.completed"),
					listing: None,
				},
			},
		];

		Ok(HomeLayout { components })
	}
}
