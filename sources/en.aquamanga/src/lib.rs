#![no_std]
use aidoku::{
	Chapter, ContentRating, FilterItem, FilterValue, HomeComponent, HomeComponentValue, HomeLayout,
	Link, Listing, ListingKind, Manga, MangaPageResult, MangaStatus, MangaWithChapter, Result,
	Source, Viewer,
	alloc::{String, Vec, vec},
	imports::net::{Request, RequestError, Response},
	prelude::*,
};
use madara::{
	Impl, LoadMoreStrategy, Madara,
	helpers::{self, ElementImageAttr},
};

const BASE_URL: &str = "https://aquareader.net";

const BROWSE_GENRES: &[(&str, &str)] = &[
	("Action", "action"),
	("Adventure", "adventure"),
	("Comedy", "comedy"),
	("Drama", "drama"),
	("Fantasy", "fantasy"),
	("Romance", "romance"),
	("Isekai", "isekai"),
	("Supernatural", "supernatural"),
	("Horror", "horror"),
	("Mystery", "mystery"),
	("Martial Arts", "martial-arts"),
	("Regression", "regression"),
	("Reincarnation", "reincarnation"),
	("Survival", "survival"),
	("System", "system"),
];

struct AquaManga;

impl Impl for AquaManga {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> madara::Params {
		madara::Params {
			base_url: BASE_URL.into(),
			use_load_more_request: LoadMoreStrategy::Never,
			default_viewer: Viewer::Webtoon,
			..Default::default()
		}
	}

	fn get_manga_status(&self, str: &str) -> MangaStatus {
		match str.to_ascii_lowercase().as_str() {
			"ongoing" | "serialization" => MangaStatus::Ongoing,
			"completed" => MangaStatus::Completed,
			"cancelled" | "dropped" => MangaStatus::Cancelled,
			_ => MangaStatus::Unknown,
		}
	}

	fn get_manga_content_rating(
		&self,
		_html: &aidoku::imports::html::Document,
		manga: &Manga,
	) -> ContentRating {
		if let Some(ref tags) = manga.tags
			&& tags.iter().any(|t| t.eq_ignore_ascii_case("ecchi"))
		{
			return ContentRating::Suggestive;
		}
		ContentRating::Safe
	}

	fn get_manga_list(
		&self,
		_params: &madara::Params,
		listing: Listing,
		page: i32,
	) -> Result<MangaPageResult> {
		let url = match listing.name.as_str() {
			"Latest Updates" => {
				if page <= 1 {
					format!("{}/", BASE_URL)
				} else {
					format!("{}/page/{}/", BASE_URL, page)
				}
			}
			"Popular" => {
				if page <= 1 {
					format!("{}/manga/?m_orderby=views", BASE_URL)
				} else {
					format!("{}/manga/page/{}/?m_orderby=views", BASE_URL, page)
				}
			}
			"New Releases" => {
				if page <= 1 {
					format!("{}/manga/?m_orderby=new-manga", BASE_URL)
				} else {
					format!("{}/manga/page/{}/?m_orderby=new-manga", BASE_URL, page)
				}
			}
			"Trending" => {
				if page <= 1 {
					format!("{}/manga/?m_orderby=trending", BASE_URL)
				} else {
					format!("{}/manga/page/{}/?m_orderby=trending", BASE_URL, page)
				}
			}
			"Latest Completed" => {
				if page <= 1 {
					format!(
						"{}/page/1/?s&post_type=wp-manga&status[]=end&m_orderby=modified",
						BASE_URL
					)
				} else {
					format!(
						"{}/page/{}/?s&post_type=wp-manga&status[]=end&m_orderby=modified",
						BASE_URL, page
					)
				}
			}
			_ => format!("{}/", BASE_URL),
		};
		parse_manga_list(&url)
	}

	fn get_home(&self, params: &madara::Params) -> Result<HomeLayout> {
		let make_listing = |id: &str, name: &str| Listing {
			id: String::from(id),
			name: String::from(name),
			kind: ListingKind::Default,
		};

		let manga_to_links =
			|entries: Vec<Manga>| -> Vec<Link> { entries.into_iter().map(Into::into).collect() };

		let responses: [core::result::Result<Response, RequestError>; 5] = Request::send_all([
			Request::get(format!("{}/manga/?m_orderby=views", BASE_URL))?,
			Request::get(format!("{}/manga/?m_orderby=new-manga", BASE_URL))?,
			Request::get(format!("{}/", BASE_URL))?,
			Request::get(format!("{}/manga/?m_orderby=trending", BASE_URL))?,
			Request::get(format!(
				"{}/page/1/?s&post_type=wp-manga&status[]=end&m_orderby=modified",
				BASE_URL
			))?,
		])
		.try_into()
		.expect("requests vec length should be 5");

		let [
			popular_resp,
			new_resp,
			latest_resp,
			trending_resp,
			completed_resp,
		] = responses;

		let top_popular: Vec<Manga> = popular_resp
			.ok()
			.and_then(|r| r.get_html().ok())
			.map(|html| parse_manga_html(html).entries)
			.unwrap_or_default()
			.into_iter()
			.take(5)
			.collect();

		let detail_responses = Request::send_all(
			top_popular
				.iter()
				.map(|m| Request::get(format!("{}{}", BASE_URL, m.key)).unwrap()),
		);

		let mut popular_entries: Vec<Manga> = Vec::new();
		for (manga, detail_resp) in top_popular.into_iter().zip(detail_responses.into_iter()) {
			if let Some(detail_html) = detail_resp.ok().and_then(|r| r.get_html().ok()) {
				let description = detail_html
					.select_first(&params.details_description_selector)
					.and_then(|el| el.text());
				let cover = detail_html
					.select_first(&params.details_cover_selector)
					.and_then(|img| img.img_attr(false))
					.or(manga.cover);
				let tags: Option<Vec<String>> = detail_html
					.select(&params.details_tag_selector)
					.map(|els| {
						els.filter_map(|el| el.text())
							.filter(|s: &String| !s.is_empty())
							.collect()
					})
					.filter(|v: &Vec<String>| !v.is_empty());
				popular_entries.push(Manga {
					key: manga.key,
					title: manga.title,
					cover,
					description,
					tags,
					..Default::default()
				});
			} else {
				popular_entries.push(manga);
			}
		}

		let mut components: Vec<HomeComponent> = Vec::new();

		components.push(HomeComponent {
			title: Some(String::from("Popular Series")),
			value: HomeComponentValue::BigScroller {
				entries: popular_entries,
				auto_scroll_interval: Some(5.0),
			},
			..Default::default()
		});

		if let Some(html) = new_resp.ok().and_then(|r| r.get_html().ok()) {
			components.push(HomeComponent {
				title: Some(String::from("New Comic Releases")),
				value: HomeComponentValue::Scroller {
					entries: manga_to_links(parse_manga_html(html).entries),
					listing: Some(make_listing("New Releases", "New Releases")),
				},
				..Default::default()
			});
		}

		let latest_html = latest_resp?.get_html()?;
		let latest_entries: Vec<MangaWithChapter> = latest_html
			.select(".page-item-detail")
			.map(|items| {
				items
					.take(10)
					.filter_map(|item| {
						let href = item.select_first("a").and_then(|a| a.attr("href"))?;
						let title = item.select_first(".post-title").and_then(|el| el.text())?;
						let key = strip_base(href);
						let cover = item.select_first("img").and_then(|img| img.img_attr(false));
						let chapter_key = strip_base(
							item.select_first(".chapter-item a")
								.and_then(|a| a.attr("href"))
								.unwrap_or_default(),
						);
						let chapter_title =
							item.select_first(".chapter-item a").and_then(|a| a.text());
						let date_uploaded = item
							.select_first(".post-on .c-new-tag")
							.and_then(|a| a.attr("title"))
							.map(|s: String| helpers::parse_chapter_date(params, s.trim()));
						Some(MangaWithChapter {
							manga: Manga {
								key,
								title,
								cover,
								..Default::default()
							},
							chapter: Chapter {
								key: chapter_key,
								title: chapter_title,
								date_uploaded,
								..Default::default()
							},
						})
					})
					.collect()
			})
			.unwrap_or_default();
		components.push(HomeComponent {
			title: Some(String::from("Latest Updates")),
			value: HomeComponentValue::MangaChapterList {
				page_size: Some(5),
				entries: latest_entries,
				listing: Some(make_listing("Latest Updates", "Latest Updates")),
			},
			..Default::default()
		});

		if let Some(html) = trending_resp.ok().and_then(|r| r.get_html().ok()) {
			components.push(HomeComponent {
				title: Some(String::from("Trending")),
				value: HomeComponentValue::Scroller {
					entries: manga_to_links(parse_manga_html(html).entries),
					listing: Some(make_listing("Trending", "Trending")),
				},
				..Default::default()
			});
		}

		if let Some(html) = completed_resp.ok().and_then(|r| r.get_html().ok()) {
			components.push(HomeComponent {
				title: Some(String::from("Latest Completed")),
				value: HomeComponentValue::Scroller {
					entries: manga_to_links(parse_manga_html(html).entries),
					listing: Some(make_listing("Latest Completed", "Latest Completed")),
				},
				..Default::default()
			});
		}

		let mut browse_items: Vec<FilterItem> = vec![
			FilterItem {
				title: String::from("Manga"),
				values: Some(vec![FilterValue::MultiSelect {
					id: String::from("genre[]"),
					included: vec![String::from("manga")],
					excluded: vec![],
				}]),
			},
			FilterItem {
				title: String::from("Manhwa"),
				values: Some(vec![FilterValue::MultiSelect {
					id: String::from("genre[]"),
					included: vec![String::from("manhwa")],
					excluded: vec![],
				}]),
			},
			FilterItem {
				title: String::from("Manhua"),
				values: Some(vec![FilterValue::MultiSelect {
					id: String::from("genre[]"),
					included: vec![String::from("manhua")],
					excluded: vec![],
				}]),
			},
			FilterItem {
				title: String::from("Completed"),
				values: Some(vec![FilterValue::Select {
					id: String::from("status[]"),
					value: String::from("Completed"),
				}]),
			},
			FilterItem {
				title: String::from("Ongoing"),
				values: Some(vec![FilterValue::Select {
					id: String::from("status[]"),
					value: String::from("Ongoing"),
				}]),
			},
		];

		for (title, slug) in BROWSE_GENRES {
			browse_items.push(FilterItem {
				title: String::from(*title),
				values: Some(vec![FilterValue::MultiSelect {
					id: String::from("genre[]"),
					included: vec![String::from(*slug)],
					excluded: vec![],
				}]),
			});
		}

		components.push(HomeComponent {
			title: Some(String::from("Browse")),
			value: HomeComponentValue::Filters(browse_items),
			..Default::default()
		});

		Ok(HomeLayout { components })
	}
}

fn strip_base(s: String) -> String {
	s.strip_prefix(BASE_URL).map(String::from).unwrap_or(s)
}

fn parse_manga_html(html: aidoku::imports::html::Document) -> MangaPageResult {
	let mut entries: Vec<Manga> = Vec::new();

	if let Some(items) = html.select(".col-6.col-md-3") {
		for item in items {
			let Some(link) = item.select_first(".item-thumb a") else {
				continue;
			};
			let Some(href) = link.attr("href") else {
				continue;
			};
			let Some(title) = link.attr("title") else {
				continue;
			};
			let key = strip_base(href);
			let cover = item
				.select_first(".item-thumb img")
				.and_then(|img| img.img_attr(false));
			entries.push(Manga {
				key,
				title,
				cover,
				..Default::default()
			});
		}
	}

	if entries.is_empty()
		&& let Some(items) = html.select(".c-tabs-item__content")
	{
		for item in items {
			let Some(href) = item
				.select_first(".tab-thumb a")
				.and_then(|a| a.attr("href"))
			else {
				continue;
			};
			let Some(title) = item.select_first(".post-title a").and_then(|el| el.text()) else {
				continue;
			};
			let key = strip_base(href);
			let cover = item
				.select_first(".tab-thumb img")
				.and_then(|img| img.img_attr(false));
			entries.push(Manga {
				key,
				title,
				cover,
				..Default::default()
			});
		}
	}

	if entries.is_empty()
		&& let Some(items) = html.select(".page-item-detail")
	{
		for item in items {
			let Some(href) = item.select_first("a").and_then(|a| a.attr("href")) else {
				continue;
			};
			let Some(title) = item.select_first(".post-title").and_then(|el| el.text()) else {
				continue;
			};
			let key = strip_base(href);
			let cover = item.select_first("img").and_then(|img| img.img_attr(false));
			entries.push(Manga {
				key,
				title,
				cover,
				..Default::default()
			});
		}
	}

	let has_next_page = html.select_first("a[class*='next']").is_some();
	MangaPageResult {
		entries,
		has_next_page,
	}
}

fn parse_manga_list(url: &str) -> Result<MangaPageResult> {
	Ok(parse_manga_html(Request::get(url)?.html()?))
}

register_source!(
	Madara<AquaManga>,
	ListingProvider,
	Home,
	MigrationHandler,
	ImageRequestProvider
);
