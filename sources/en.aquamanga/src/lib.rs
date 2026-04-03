#![no_std]
use aidoku::{
	Chapter, ContentRating, FilterItem, FilterValue, HomeComponent, HomeComponentValue, HomeLayout,
	Link, LinkValue, Listing, ListingKind, Manga, MangaPageResult, MangaStatus, MangaWithChapter,
	Result, Source, Viewer,
	alloc::string::ToString,
	alloc::{String, Vec, vec},
	imports::net::Request,
	prelude::*,
};
use madara::{
	Impl, LoadMoreStrategy, Madara,
	helpers::{self, ElementImageAttr},
};

const BASE_URL: &str = "https://aquareader.net";

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
		match str {
			"OnGoing" | "Ongoing" | "ongoing" | "Serialization" => MangaStatus::Ongoing,
			"Completed" | "completed" => MangaStatus::Completed,
			"Cancelled" | "cancelled" | "Dropped" => MangaStatus::Cancelled,
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

	fn get_search_manga_list(
		&self,
		_params: &madara::Params,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		use aidoku::helpers::uri::QueryParameters;
		let mut qs = QueryParameters::new();
		qs.push("s", Some(&query.unwrap_or_default()));
		qs.push("post_type", Some("wp-manga"));

		for filter in filters {
			match filter {
				FilterValue::Sort { id, index, .. } if id == "type" => {
					let genre = match index {
						1 => Some("manga"),
						2 => Some("manhwa"),
						3 => Some("manhua"),
						_ => None,
					};
					if let Some(g) = genre {
						qs.push("genre[]", Some(g));
					}
				}
				FilterValue::Sort { id, index, .. } if id == "sort" => {
					let order = match index {
						1 => "alphabet",
						2 => "new-manga",
						3 => "latest",
						4 => "rating",
						5 => "trending",
						_ => "",
					};
					if !order.is_empty() {
						qs.push("m_orderby", Some(order));
					}
				}
				FilterValue::Select { id, value } if id == "status" => {
					let status = match value.as_str() {
						"Completed" => Some("end"),
						"Ongoing" => Some("on-going"),
						_ => None,
					};
					if let Some(s) = status {
						qs.push("status[]", Some(s));
					}
				}
				FilterValue::MultiSelect { id, included, .. } if id == "genre[]" => {
					for genre_id in included {
						qs.push("genre[]", Some(&genre_id));
					}
				}
				_ => {}
			}
		}

		let page_str;
		let url = if page <= 1 {
			format!("{}/?{qs}", BASE_URL)
		} else {
			page_str = page.to_string();
			let _ = &page_str;
			format!("{}/page/{page}/?{qs}", BASE_URL)
		};

		let html = Request::get(&url)?.html()?;
		let mut entries: Vec<Manga> = Vec::new();

		if let Some(items) = html.select(".c-tabs-item__content") {
			for item in items {
				let Some(href) = item
					.select_first(".tab-thumb a")
					.and_then(|a| a.attr("href"))
				else {
					continue;
				};
				let Some(title) = item.select_first(".post-title a").and_then(|el| el.text())
				else {
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

		let has_next_page = html.select_first("a[class*='next']").is_some();
		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
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

		let manga_to_links = |entries: Vec<Manga>| -> Vec<Link> {
			entries
				.into_iter()
				.map(|m| Link {
					title: m.title,
					image_url: m.cover,
					value: Some(LinkValue::Manga(Manga {
						key: m.key,
						..Default::default()
					})),
					..Default::default()
				})
				.collect()
		};

		let mut components: Vec<HomeComponent> = Vec::new();

		let latest_html = Request::get(format!("{}/", BASE_URL))?.html()?;
		let mut latest_entries: Vec<MangaWithChapter> = Vec::new();
		if let Some(items) = latest_html.select(".page-item-detail") {
			for item in items.take(10) {
				let key = strip_base(
					item.select_first("a")
						.and_then(|a| a.attr("href"))
						.unwrap_or_default(),
				);
				let title = item
					.select_first(".post-title")
					.and_then(|el| el.text())
					.unwrap_or_default();
				let cover = item.select_first("img").and_then(|img| img.img_attr(false));
				let chapter_key = strip_base(
					item.select_first(".chapter-item a")
						.and_then(|a| a.attr("href"))
						.unwrap_or_default(),
				);
				let chapter_title = item.select_first(".chapter-item a").and_then(|a| a.text());
				let date_uploaded = item
					.select_first(".post-on .c-new-tag")
					.and_then(|a| a.attr("title"))
					.map(|s: String| helpers::parse_chapter_date(params, s.trim()));
				if !key.is_empty() && !title.is_empty() {
					latest_entries.push(MangaWithChapter {
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
					});
				}
			}
		}
		components.push(HomeComponent {
			title: Some(String::from("Latest Updates")),
			value: HomeComponentValue::MangaChapterList {
				page_size: Some(5),
				entries: latest_entries,
				listing: Some(make_listing("Latest Updates", "Latest Updates")),
			},
			..Default::default()
		});

		let mut popular_entries: Vec<Manga> = Vec::new();
		if let Ok(popular) = parse_manga_list(&format!("{}/manga/?m_orderby=views", BASE_URL)) {
			for manga in popular.entries.into_iter().take(5) {
				let detail_url = format!("{}{}", BASE_URL, manga.key);
				if let Ok(detail_html) = Request::get(&detail_url).and_then(|r| r.html()) {
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
		}
		components.push(HomeComponent {
			title: Some(String::from("Popular Series")),
			value: HomeComponentValue::BigScroller {
				entries: popular_entries,
				auto_scroll_interval: Some(5.0),
			},
			..Default::default()
		});

		if let Ok(result) = parse_manga_list(&format!("{}/manga/?m_orderby=new-manga", BASE_URL)) {
			components.push(HomeComponent {
				title: Some(String::from("New Comic Releases")),
				value: HomeComponentValue::Scroller {
					entries: manga_to_links(result.entries),
					listing: Some(make_listing("New Releases", "New Releases")),
				},
				..Default::default()
			});
		}

		if let Ok(result) = parse_manga_list(&format!("{}/manga/?m_orderby=trending", BASE_URL)) {
			components.push(HomeComponent {
				title: Some(String::from("Trending")),
				value: HomeComponentValue::Scroller {
					entries: manga_to_links(result.entries),
					listing: Some(make_listing("Trending", "Trending")),
				},
				..Default::default()
			});
		}

		if let Ok(result) = parse_manga_list(&format!(
			"{}/page/1/?s&post_type=wp-manga&status[]=end&m_orderby=modified",
			BASE_URL
		)) {
			components.push(HomeComponent {
				title: Some(String::from("Latest Completed")),
				value: HomeComponentValue::Scroller {
					entries: manga_to_links(result.entries),
					listing: Some(make_listing("Latest Completed", "Latest Completed")),
				},
				..Default::default()
			});
		}

		components.push(HomeComponent {
			title: Some(String::from("Browse")),
			value: HomeComponentValue::Filters(vec![
				FilterItem {
					title: String::from("Manga"),
					values: Some(vec![FilterValue::Sort {
						id: String::from("type"),
						index: 1,
						ascending: false,
					}]),
				},
				FilterItem {
					title: String::from("Manhwa"),
					values: Some(vec![FilterValue::Sort {
						id: String::from("type"),
						index: 2,
						ascending: false,
					}]),
				},
				FilterItem {
					title: String::from("Manhua"),
					values: Some(vec![FilterValue::Sort {
						id: String::from("type"),
						index: 3,
						ascending: false,
					}]),
				},
				FilterItem {
					title: String::from("Completed"),
					values: Some(vec![FilterValue::Select {
						id: String::from("status"),
						value: String::from("Completed"),
					}]),
				},
				FilterItem {
					title: String::from("Ongoing"),
					values: Some(vec![FilterValue::Select {
						id: String::from("status"),
						value: String::from("Ongoing"),
					}]),
				},
				FilterItem {
					title: String::from("Action"),
					values: Some(vec![FilterValue::MultiSelect {
						id: String::from("genre[]"),
						included: vec![String::from("action")],
						excluded: vec![],
					}]),
				},
				FilterItem {
					title: String::from("Adventure"),
					values: Some(vec![FilterValue::MultiSelect {
						id: String::from("genre[]"),
						included: vec![String::from("adventure")],
						excluded: vec![],
					}]),
				},
				FilterItem {
					title: String::from("Comedy"),
					values: Some(vec![FilterValue::MultiSelect {
						id: String::from("genre[]"),
						included: vec![String::from("comedy")],
						excluded: vec![],
					}]),
				},
				FilterItem {
					title: String::from("Drama"),
					values: Some(vec![FilterValue::MultiSelect {
						id: String::from("genre[]"),
						included: vec![String::from("drama")],
						excluded: vec![],
					}]),
				},
				FilterItem {
					title: String::from("Fantasy"),
					values: Some(vec![FilterValue::MultiSelect {
						id: String::from("genre[]"),
						included: vec![String::from("fantasy")],
						excluded: vec![],
					}]),
				},
				FilterItem {
					title: String::from("Romance"),
					values: Some(vec![FilterValue::MultiSelect {
						id: String::from("genre[]"),
						included: vec![String::from("romance")],
						excluded: vec![],
					}]),
				},
				FilterItem {
					title: String::from("Isekai"),
					values: Some(vec![FilterValue::MultiSelect {
						id: String::from("genre[]"),
						included: vec![String::from("isekai")],
						excluded: vec![],
					}]),
				},
				FilterItem {
					title: String::from("Supernatural"),
					values: Some(vec![FilterValue::MultiSelect {
						id: String::from("genre[]"),
						included: vec![String::from("supernatural")],
						excluded: vec![],
					}]),
				},
				FilterItem {
					title: String::from("Horror"),
					values: Some(vec![FilterValue::MultiSelect {
						id: String::from("genre[]"),
						included: vec![String::from("horror")],
						excluded: vec![],
					}]),
				},
				FilterItem {
					title: String::from("Mystery"),
					values: Some(vec![FilterValue::MultiSelect {
						id: String::from("genre[]"),
						included: vec![String::from("mystery")],
						excluded: vec![],
					}]),
				},
				FilterItem {
					title: String::from("Martial Arts"),
					values: Some(vec![FilterValue::MultiSelect {
						id: String::from("genre[]"),
						included: vec![String::from("martial-arts")],
						excluded: vec![],
					}]),
				},
				FilterItem {
					title: String::from("Regression"),
					values: Some(vec![FilterValue::MultiSelect {
						id: String::from("genre[]"),
						included: vec![String::from("regression")],
						excluded: vec![],
					}]),
				},
				FilterItem {
					title: String::from("Reincarnation"),
					values: Some(vec![FilterValue::MultiSelect {
						id: String::from("genre[]"),
						included: vec![String::from("reincarnation")],
						excluded: vec![],
					}]),
				},
				FilterItem {
					title: String::from("Survival"),
					values: Some(vec![FilterValue::MultiSelect {
						id: String::from("genre[]"),
						included: vec![String::from("survival")],
						excluded: vec![],
					}]),
				},
				FilterItem {
					title: String::from("System"),
					values: Some(vec![FilterValue::MultiSelect {
						id: String::from("genre[]"),
						included: vec![String::from("system")],
						excluded: vec![],
					}]),
				},
			]),
			..Default::default()
		});

		Ok(HomeLayout { components })
	}
}

fn strip_base(s: String) -> String {
	s.strip_prefix(BASE_URL).map(String::from).unwrap_or(s)
}

fn parse_manga_list(url: &str) -> Result<MangaPageResult> {
	let html = Request::get(url)?.html()?;
	let mut entries: Vec<Manga> = Vec::new();

	// manga listing pages (/manga/?m_orderby=...)
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

	// search/filter pages (use tab-thumb + post-title)
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

	// homepage latest updates
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
	Ok(MangaPageResult {
		entries,
		has_next_page,
	})
}

register_source!(
	Madara<AquaManga>,
	ListingProvider,
	Home,
	MigrationHandler,
	ImageRequestProvider
);
