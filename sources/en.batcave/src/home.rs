use crate::{BASE_URL, BatCave, BatCaveHtml};
use aidoku::{
	Home, HomeComponent, HomeLayout, HomePartialResult, Link, Manga, Result,
	alloc::{Box, Vec, string::ToString},
	imports::{html::Document, net::Request, std::send_partial_result},
	prelude::*,
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
						.filter_map(|element| {
							let title = element
								.select_first("div > p")
								.and_then(|x| x.text())
								.unwrap_or_default();

							let cover = element
								.select_first("img")
								.and_then(|x| x.attr("abs:data-src"));

							let url = element.attr("abs:href");
							let key = url.clone()?.strip_prefix(BASE_URL)?.to_string();

							Some(Manga {
								key,
								cover,
								title,
								url,
								..Default::default()
							})
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

		fn get_home_series_worth_starting(html: &Document) -> Option<HomeComponent> {
			let section = html.select_first("section.sect--worth-starting")?;
			let title = section.select_first(".sect__title").and_then(|x| x.text());

			let entries = section
				.select(".sect__content > a.grid-item")
				.map(|elements| {
					elements
						.filter_map(|element| {
							let title = element
								.select_first(".poster__title")
								.and_then(|x| x.text())
								.unwrap_or_default();
							let cover = element
								.select_first("img")
								.and_then(|x| x.attr("abs:data-src"));
							let url = element.attr("abs:href");
							let key = url.clone()?.strip_prefix(BASE_URL)?.to_string();

							Some(Manga {
								key,
								cover,
								title,
								url,
								..Default::default()
							})
						})
						.map(Into::into)
						.collect::<Vec<Link>>()
				})
				.unwrap_or_default();

			if entries.is_empty() {
				return None;
			}

			Some(HomeComponent {
				title,
				value: aidoku::HomeComponentValue::Scroller {
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
							.filter_map(|element| {
								let title = element
									.select_first(".popular__title")
									.and_then(|x| x.text())
									.unwrap_or_default();

								let cover = element
									.select_first("img")
									.and_then(|x| x.attr("abs:data-src"));

								let url = element.attr("abs:href");
								let key = url.clone()?.strip_prefix(BASE_URL)?.to_string();

								Some(Manga {
									key,
									cover,
									title,
									url,
									..Default::default()
								})
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
		let html = Request::get(BASE_URL)?.batcave_html()?;

		let component_fns: &[ComponentBuilderFn; 4] = &[
			Box::new(get_home_hot_releases),
			Box::new(get_home_series_worth_starting),
			// get_side_block(1), "Free Steam games"
			get_side_block(2),
			get_side_block(3),
		];

		for component_fn in component_fns {
			if let Some(component) = component_fn(&html) {
				send_partial_result(&HomePartialResult::Component(component));
			}
		}

		Ok(HomeLayout::default())
	}
}
