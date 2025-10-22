use crate::{BatCave, BASE_URL};
use aidoku::{
	alloc::{Box, String, Vec},
	imports::std::send_partial_result,
	imports::{html::Document, net::Request, std::parse_date},
	prelude::*,
	Chapter, Home, HomeComponent, HomeLayout, HomePartialResult, Link, Manga, MangaWithChapter,
	Result,
};

type ComponentBuilderFn = Box<dyn Fn(&Document) -> Option<HomeComponent>>;

impl Home for BatCave {
	fn get_home(&self) -> Result<HomeLayout> {
		fn get_home_hot_releases(html: &Document) -> Option<HomeComponent> {
			let title = html
				.select_first(".sect--hot > .sect__title")
				.and_then(|x| x.text());

			let entries = html
				.select("section.sect--hot > .sect__content > a.grid-item")
				.map(|elements| {
					elements
						.map(|element| {
							let title = element
								.select_first("div > p")
								.and_then(|x| x.text())
								.unwrap_or_default();

							let cover = element
								.select_first("img")
								.and_then(|x| x.attr("abs:data-src"));

							let url = element.attr("abs:href");

							Manga {
								key: url.clone().unwrap_or_default(),
								cover,
								title,
								url,
								..Default::default()
							}
						})
						.map(Into::into)
						.collect::<Vec<Link>>()
				})
				.unwrap_or_default();

			if !entries.is_empty() {
				Some(HomeComponent {
					title,
					value: aidoku::HomeComponentValue::Scroller {
						entries,
						listing: None,
					},
					..Default::default()
				})
			} else {
				None
			}
		}

		fn get_home_newest_releases(html: &Document) -> Option<HomeComponent> {
			let title = html
				.select_first(".sect--latest > .sect__title")
				.and_then(|x| x.text());

			let entries = html
				.select(".sect--latest > .sect__content > li.latest")
				.map(|elements| {
					elements
						.map(|element| {
							let manga_url = element
								.select_first(".latest__title")
								.and_then(|x| x.attr("abs:href"));

							let chapter_url = element
								.select_first(".latest__chapter > a")
								.and_then(|x| x.attr("abs:href"));

							let cover = element
								.select_first(".latest__img > img")
								.and_then(|x| x.attr("abs:src"));

							let manga_title = element
								.select_first(".latest__title")
								.and_then(|x| x.text())
								.unwrap_or_default();

							let details_text = element
								.select_first(".latest__chapter")
								.and_then(|x| x.text());

							let mut date_uploaded = None;
							let mut chapter_title = None;
							let mut chapter_number = None;

							if let Some(text) = details_text {
								let parts = text.splitn(2, " - ").collect::<Vec<&str>>();
								if parts.len() == 2 {
									date_uploaded = parse_date(parts[0].trim(), "dd.MM.yyyy");

									chapter_title = parts[1]
										.strip_prefix(&manga_title)
										.map(str::trim)
										.map(String::from);

									if let Some(idx) = parts[1].find('#') {
										chapter_number = parts[1][idx + 1..].parse::<f32>().ok();
									}
								}
							}

							MangaWithChapter {
								manga: Manga {
									key: manga_url.clone().unwrap_or_default(),
									cover,
									title: manga_title,
									url: manga_url,
									..Default::default()
								},
								chapter: Chapter {
									key: chapter_url.clone().unwrap_or_default(),
									url: chapter_url,
									title: chapter_title,
									chapter_number,
									date_uploaded,
									..Default::default()
								},
							}
						})
						.collect::<Vec<MangaWithChapter>>()
				})
				.unwrap_or_default();

			Some(HomeComponent {
				title,
				value: aidoku::HomeComponentValue::MangaChapterList {
					page_size: Some(6),
					entries,
					listing: None,
				},
				..Default::default()
			})
		}

		fn get_side_block(index: i32) -> ComponentBuilderFn {
			Box::new(move |html: &Document| {
				let block = html.select_first(format!(".side-block:nth-of-type({})", index))?;
				let title = block.select_first(".side-block__title")?.text();

				let entries = block
					.select(".side-block__content > a")
					.map(|elements| {
						elements
							.map(|element| {
								let title = element
									.select_first(".popular__title")
									.and_then(|x| x.text())
									.unwrap_or_default();

								let cover = element
									.select_first("img")
									.and_then(|x| x.attr("abs:data-src"));

								let url = element.attr("abs:href");

								Manga {
									key: url.clone().unwrap_or_default(),
									cover,
									title,
									url,
									..Default::default()
								}
							})
							.map(Into::into)
							.collect::<Vec<Link>>()
					})
					.unwrap_or_default();

				Some(HomeComponent {
					title,
					value: aidoku::HomeComponentValue::MangaList {
						ranking: false,
						entries,
						listing: None,
						page_size: None,
					},
					..Default::default()
				})
			})
		}
		let html = Request::get(BASE_URL)?.html()?;

		let component_fns: &[ComponentBuilderFn; 4] = &[
			Box::new(get_home_hot_releases),
			Box::new(get_home_newest_releases),
			get_side_block(1),
			get_side_block(2),
		];

		for component_fn in component_fns {
			if let Some(component) = component_fn(&html) {
				send_partial_result(&HomePartialResult::Component(component));
			}
		}

		Ok(HomeLayout::default())
	}
}
