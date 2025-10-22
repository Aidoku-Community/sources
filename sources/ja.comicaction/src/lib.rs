#![no_std]
use aidoku::{
	HomeComponent, HomeLayout, Link, LinkValue, Listing, Manga, MangaPageResult, Result, Source,
	alloc::{String, Vec},
	imports::net::Request,
	prelude::*,
};
use gigaviewer::{GigaViewer, Impl, Params};

const BASE_URL: &str = "https://comic-action.com";
const CDN_URL: &str = "https://cdn-img.comic-action.com/public/page";

struct ComicAction;

impl Impl for ComicAction {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			cdn_url: CDN_URL.into(),
			is_paginated: true,
			search_item_selector: "section > ul > li".into(),
			search_item_title_selector: "div > p".into(),
			..Default::default()
		}
	}

	fn get_manga_list(
		&self,
		params: &Params,
		listing: Listing,
		_page: i32,
	) -> Result<MangaPageResult> {
		let html = Request::get(format!("{}/{}", params.base_url, listing.id))?.html()?;

		let entries = gigaviewer::parser::parse_response(
			&html,
			&params.base_url,
			"section > ul > li",
			"h3",
			"img",
			"src",
			Some("h3 + p"),
			None,
		);

		Ok(MangaPageResult {
			entries,
			has_next_page: false,
		})
	}

	fn get_home(&self, _params: &Params) -> Result<HomeLayout> {
		let html = Request::get(BASE_URL)?.html()?;

		let mut components = Vec::new();

		let manga_prefix = format!("{BASE_URL}/episode");
		let links: Vec<Link> = html
			.select_first(".swiper")
			.and_then(|x| {
				Some(
					x.select(".swiper-slide:not(.swiper-slide-duplicate) a")?
						.filter_map(|e| {
							let image = e.select("img")?.next_back()?.attr("src")?;
							let url = e.attr("href")?;
							let value = if url.starts_with(&manga_prefix) {
								let key = url.strip_prefix(BASE_URL).map(String::from)?;
								LinkValue::Manga(Manga {
									key,
									..Default::default()
								})
							} else {
								LinkValue::Url(url)
							};
							let link = Link {
								title: String::default(),
								image_url: Some(image),
								value: Some(value),
								..Default::default()
							};
							Some(link)
						})
						.collect(),
				)
			})
			.unwrap_or_default();
		if !links.is_empty() {
			components.push(HomeComponent {
				title: None,
				subtitle: None,
				value: aidoku::HomeComponentValue::ImageScroller {
					links,
					auto_scroll_interval: Some(5.0),
					width: Some(340),
					height: Some(170),
				},
			});
		}

		if let Some(sections) = html.select("#grouped-series > section") {
			for section in sections {
				let titles = section.select_first("h2").map(|h2| {
					let mut children = h2.children();
					let title = children.first().and_then(|child| child.text());
					let subtitle = children.next_back().and_then(|child| child.text());
					(title, subtitle)
				});
				let Some((title, subtitle)) = titles else {
					continue;
				};
				components.push(HomeComponent {
					title,
					subtitle,
					value: aidoku::HomeComponentValue::Scroller {
						entries: section
							.select("ul > li")
							.map(|x| {
								x.filter_map(|element| {
									let link = element.select_first("a")?;
									let key = link
										.attr("href")?
										.strip_prefix(BASE_URL)
										.map(String::from)?;
									let title = link.attr("data-series-name")?;
									let cover =
										element.select_first("img").and_then(|el| el.attr("src"));
									let subtitle = element
										.select_first("p[class^=\"Common_copy_\"]")
										.and_then(|el| el.text());

									Some(Link {
										title: title.clone(),
										subtitle,
										image_url: cover.clone(),
										value: Some(LinkValue::Manga(Manga {
											key,
											title,
											cover,
											..Default::default()
										})),
									})
								})
								.collect()
							})
							.unwrap_or_default(),
						listing: None,
					},
				});
			}
		}

		Ok(HomeLayout { components })
	}
}

register_source!(
	GigaViewer<ComicAction>,
	Home,
	ListingProvider,
	PageImageProcessor,
	BasicLoginHandler,
	NotificationHandler,
	DeepLinkHandler
);
