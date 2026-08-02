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
use madara::{Impl, LoadMoreStrategy, Madara, helpers::ElementImageAttr};

const BASE_URL: &str = "https://aquareader.org";

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
	("Manhwa", "manhwa"),
	("Manhua", "manhua"),
	("Manga", "manga"),
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
			use_new_chapter_endpoint: true,
			default_viewer: Viewer::Webtoon,
			details_title_selector: "h1.aqua-series-info__title".into(),
			details_cover_selector: "[class*='cover'] img, [class*='poster'] img".into(),
			details_author_selector: ".aqua-series-info__creator-value".into(),
			details_artist_selector:
				".aqua-series-info__creator-value ~ .aqua-series-info__creator-value".into(),
			details_description_selector: ".aqua-series-synopsis".into(),
			details_tag_selector: ".aqua-series-genres a".into(),
			details_status_selector: ".aqua-series-meta__status".into(),
			details_type_selector: ".aqua-series-meta__type".into(),
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
					format!("{}/manga/?m_orderby=latest", BASE_URL)
				} else {
					format!("{}/manga/page/{}/?m_orderby=latest", BASE_URL, page)
				}
			}
			"Popular Today" => {
				if page <= 1 {
					format!("{}/manga/?m_orderby=trending", BASE_URL)
				} else {
					format!("{}/manga/page/{}/?m_orderby=trending", BASE_URL, page)
				}
			}
			"New Series" => {
				if page <= 1 {
					format!("{}/manga/?m_orderby=new-manga", BASE_URL)
				} else {
					format!("{}/manga/page/{}/?m_orderby=new-manga", BASE_URL, page)
				}
			}
			"All Series" | _ => {
				if page <= 1 {
					format!("{}/manga/", BASE_URL)
				} else {
					format!("{}/manga/page/{}/", BASE_URL, page)
				}
			}
		};
		parse_manga_list(&url)
	}

	fn get_search_manga_list(
		&self,
		_params: &madara::Params,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		use aidoku::helpers::uri::QueryParameters;

		let mut orderby: Option<&str> = None;
		let mut status: Option<&str> = None;
		let mut genres: Vec<String> = Vec::new();
		let query_str = query.unwrap_or_default();
		let has_search = !query_str.is_empty();

		for filter in filters {
			match filter {
				FilterValue::Sort { id, index, .. } if id == "m_orderby" => {
					orderby = match index {
						1 => Some("latest"),
						2 => Some("views"),
						3 => Some("new-manga"),
						4 => Some("alphabet"),
						_ => None,
					};
				}
				FilterValue::Select { id, value } if id == "manga_status" => {
					status = match value.as_str() {
						"Ongoing" => Some("on-going"),
						"Completed" => Some("end"),
						_ => None,
					};
				}
				FilterValue::MultiSelect { id, included, .. } if id == "genre[]" => {
					genres = included;
				}
				_ => {}
			}
		}

		if has_search {
			let mut qs = QueryParameters::new();
			qs.push("s", Some(&query_str));
			qs.push("post_type", Some("wp-manga"));
			if let Some(s) = status {
				qs.push("manga_status", Some(s));
			}
			for genre in &genres {
				qs.push("genre[]", Some(genre));
			}
			let page_str;
			let url = if page <= 1 {
				format!("{}/page/1/?{qs}", BASE_URL)
			} else {
				page_str = page.to_string();
				let _ = &page_str;
				format!("{}/page/{page}/?{qs}", BASE_URL)
			};
			return parse_manga_list(&url);
		}

		let mut params_vec: Vec<String> = Vec::new();
		if let Some(o) = orderby {
			params_vec.push(format!("m_orderby={}", o));
		}
		if let Some(s) = status {
			params_vec.push(format!("manga_status={}", s));
		}
		for genre in &genres {
			params_vec.push(format!("genre={}", genre));
		}

		let page_str;
		let url = if params_vec.is_empty() {
			if page <= 1 {
				format!("{}/manga/", BASE_URL)
			} else {
				page_str = page.to_string();
				let _ = &page_str;
				format!("{}/manga/page/{}/", BASE_URL, page)
			}
		} else {
			let qs = params_vec.join("&");
			if page <= 1 {
				format!("{}/manga/?{}", BASE_URL, qs)
			} else {
				page_str = page.to_string();
				let _ = &page_str;
				format!("{}/manga/page/{page}/?{}", BASE_URL, qs)
			}
		};

		parse_manga_list(&url)
	}

	fn get_home(&self, _params: &madara::Params) -> Result<HomeLayout> {
		let make_listing = |id: &str, name: &str| Listing {
			id: String::from(id),
			name: String::from(name),
			kind: ListingKind::Default,
		};

		let html = Request::get(format!("{}/", BASE_URL))?.html()?;
		let mut components: Vec<HomeComponent> = Vec::new();

		// Popular Today — BigScroller from hero slides
		let hero_entries: Vec<Manga> = html
			.select(".aqua-hero-slide")
			.map(|items| {
				items
					.filter_map(|item| {
						let href = item
							.select_first(".aqua-hero-slide__title a")
							.and_then(|a| a.attr("href"))?;
						let title = item
							.select_first(".aqua-hero-slide__title a")
							.and_then(|a| a.text())?;
						let key = strip_base(href);
						let cover = item.select_first("img").and_then(|img| img.img_attr(false));
						let description = item
							.select_first(".aqua-hero-slide__excerpt")
							.and_then(|p| p.text())
							.map(|t| String::from(t.trim()));
						let tags: Option<Vec<String>> =
							item.select(".aqua-hero-slide__genre").map(|els| {
								els.filter_map(|el| el.text())
									.map(|t| String::from(t.trim()))
									.filter(|s| !s.is_empty())
									.collect()
							});
						Some(Manga {
							key,
							title,
							cover,
							description,
							tags,
							..Default::default()
						})
					})
					.collect()
			})
			.unwrap_or_default();

		if !hero_entries.is_empty() {
			components.push(HomeComponent {
				title: Some(String::from("Popular Today")),
				value: HomeComponentValue::BigScroller {
					entries: hero_entries,
					auto_scroll_interval: Some(5.0),
				},
				..Default::default()
			});
		}

		// Latest Updates — .aqua-latest-section .aqua-manga-card
		let latest_entries: Vec<MangaWithChapter> = html
			.select(".aqua-latest-section .aqua-manga-card")
			.map(|items| {
				items
					.take(10)
					.filter_map(|card| {
						let link = card.select_first(".aqua-manga-card__cover-link")?;
						let href = link.attr("href")?;
						let key = strip_base(href);
						let cover = link.select_first("img").and_then(|img| img.img_attr(false));
						let title = card
							.select_first(".aqua-manga-card__title a")
							.and_then(|a| a.text())?;
						let chapter_pill = card.select_first("a.aqua-chapter-pill");
						let chapter_key = strip_base(
							chapter_pill
								.as_ref()
								.and_then(|a| a.attr("href"))
								.unwrap_or_default(),
						);
						let chapter_name = card
							.select_first(".aqua-chapter-pill__name")
							.and_then(|el| el.text())
							.map(|t| String::from(t.trim()));
						let date_uploaded = card
							.select_first(".aqua-chapter-pill__time")
							.and_then(|el| el.text())
							.and_then(|t| {
								use aidoku::imports::std::current_date;
								let now = current_date();
								let s = t.trim().to_ascii_lowercase();
								let num: i64 = s
									.split(|c: char| !c.is_ascii_digit())
									.find_map(|n| n.parse().ok())
									.unwrap_or(1);
								let offset = if s.ends_with('d') || s.contains("day") {
									num * 86400
								} else if s.ends_with('h') || s.contains("hour") {
									num * 3600
								} else if s.ends_with('m') || s.contains("min") {
									num * 60
								} else if s.contains("week") {
									num * 604800
								} else if s.contains("month") {
									num * 2592000
								} else {
									return None;
								};
								Some(now - offset)
							});
						Some(MangaWithChapter {
							manga: Manga {
								key,
								title: String::from(title.trim()),
								cover,
								..Default::default()
							},
							chapter: Chapter {
								key: chapter_key,
								title: chapter_name,
								date_uploaded,
								..Default::default()
							},
						})
					})
					.collect()
			})
			.unwrap_or_default();

		if !latest_entries.is_empty() {
			components.push(HomeComponent {
				title: Some(String::from("Latest Updates")),
				value: HomeComponentValue::MangaChapterList {
					page_size: Some(5),
					entries: latest_entries,
					listing: Some(make_listing("Latest Updates", "Latest Updates")),
				},
				..Default::default()
			});
		}

		// .aqua-new-series-section links
		let new_entries: Vec<Link> = html
			.select(".aqua-new-series-section a[href*='/manga/']")
			.map(|items| {
				items
					.filter_map(|a| {
						let href = a.attr("href")?;
						// Skip view-all and section links
						if href.ends_with("/manga/") || !href.contains("/manga/") {
							return None;
						}
						let key = strip_base(href.clone());
						if key.is_empty() {
							return None;
						}
						let cover = a.select_first("img").and_then(|img| img.img_attr(false));
						// Title from img alt or sibling text
						let title = a
							.select_first("img")
							.and_then(|img| img.attr("alt"))
							.or_else(|| a.select_first("[class*='title']").and_then(|el| el.text()))
							.unwrap_or_else(|| String::from(""));
						if title.is_empty() {
							return None;
						}
						Some(Link {
							title,
							image_url: cover,
							value: Some(LinkValue::Manga(Manga {
								key,
								..Default::default()
							})),
							..Default::default()
						})
					})
					.collect()
			})
			.unwrap_or_default();

		if !new_entries.is_empty() {
			components.push(HomeComponent {
				title: Some(String::from("New Series")),
				value: HomeComponentValue::Scroller {
					entries: new_entries,
					listing: Some(make_listing("New Series", "New Series")),
				},
				..Default::default()
			});
		}

		// a.aqua-popular-card
		let popular_entries: Vec<Link> = html
			.select("a.aqua-popular-card")
			.map(|items| {
				items
					.filter_map(|card| {
						let href = card.attr("href")?;
						let key = strip_base(href);
						let cover = card.select_first("img").and_then(|img| img.img_attr(false));
						let title = card
							.select_first("[class*='title']")
							.and_then(|el| el.text())?;
						Some(Link {
							title,
							image_url: cover,
							value: Some(LinkValue::Manga(Manga {
								key,
								..Default::default()
							})),
							..Default::default()
						})
					})
					.collect()
			})
			.unwrap_or_default();

		if !popular_entries.is_empty() {
			components.push(HomeComponent {
				title: Some(String::from("Popular Series")),
				value: HomeComponentValue::Scroller {
					entries: popular_entries,
					listing: Some(make_listing("Popular Today", "Popular Today")),
				},
				..Default::default()
			});
		}

		// Browse
		let mut browse_items: Vec<FilterItem> = vec![
			FilterItem {
				title: String::from("Default"),
				values: Some(vec![FilterValue::Sort {
					id: String::from("m_orderby"),
					index: 0,
					ascending: false,
				}]),
			},
			FilterItem {
				title: String::from("Latest"),
				values: Some(vec![FilterValue::Sort {
					id: String::from("m_orderby"),
					index: 1,
					ascending: false,
				}]),
			},
			FilterItem {
				title: String::from("Popular"),
				values: Some(vec![FilterValue::Sort {
					id: String::from("m_orderby"),
					index: 2,
					ascending: false,
				}]),
			},
			FilterItem {
				title: String::from("Newest"),
				values: Some(vec![FilterValue::Sort {
					id: String::from("m_orderby"),
					index: 3,
					ascending: false,
				}]),
			},
			FilterItem {
				title: String::from("Completed"),
				values: Some(vec![FilterValue::Select {
					id: String::from("manga_status"),
					value: String::from("Completed"),
				}]),
			},
			FilterItem {
				title: String::from("Ongoing"),
				values: Some(vec![FilterValue::Select {
					id: String::from("manga_status"),
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

	// Custom archive grid
	if let Some(items) = html.select("article.aqua-archive-card") {
		for item in items {
			let Some(href) = item
				.select_first(".aqua-archive-card__cover-link")
				.and_then(|a| a.attr("href"))
			else {
				continue;
			};
			let Some(title) = item
				.select_first(".aqua-archive-card__title a")
				.and_then(|a| a.text())
			else {
				continue;
			};
			let key = strip_base(href);
			let cover = item
				.select_first(".aqua-archive-card__cover")
				.and_then(|img| img.img_attr(false));
			entries.push(Manga {
				key,
				title,
				cover,
				..Default::default()
			});
		}
	}

	// Madara tab-based listing
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

	// Madara page-item-detail
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

	let has_next_page = html
		.select_first("a.next.page-numbers")
		.or_else(|| html.select_first("a[class*='next']"))
		.is_some();

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
