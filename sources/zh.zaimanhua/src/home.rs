use crate::{get_api_request, helpers::V4_API_URL, net};
use aidoku::{
	HomeComponent, HomeLayout, HomePartialResult, Listing, ListingKind, Manga, MangaStatus,
	MangaWithChapter, Result,
	alloc::{String, Vec, format, string::ToString, vec},
	imports::net::RequestError,
	imports::net::{Request, Response},
	imports::std::send_partial_result,
};

use crate::models::{ApiResponse, DetailData};

/// Build the home page layout
pub fn get_home_layout() -> Result<HomeLayout> {
	send_partial_result(&HomePartialResult::Layout(HomeLayout {
		components: vec![
			HomeComponent {
				title: None,
				subtitle: None,
				value: aidoku::HomeComponentValue::empty_image_scroller(),
			},
			HomeComponent {
				title: Some("精品推荐".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::empty_big_scroller(),
			},
			HomeComponent {
				title: Some("人气推荐".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::empty_manga_list(),
			},
			HomeComponent {
				title: Some("最近更新".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::empty_manga_chapter_list(),
			},
			HomeComponent {
				title: Some("少年漫画".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::empty_scroller(),
			},
			HomeComponent {
				title: Some("少女漫画".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::empty_scroller(),
			},
			HomeComponent {
				title: Some("男青漫画".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::empty_scroller(),
			},
			HomeComponent {
				title: Some("女青漫画".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::empty_scroller(),
			},
		],
	}));

	let recommend_url = format!("{}/comic/recommend/list", V4_API_URL);
	let latest_url = format!("{}/comic/filter/list?sortType=1&size=20&page=1", V4_API_URL);
	// Use Rank API 月榜 - 1 page = 10 items
	let rank_url = format!(
		"{}/comic/rank/list?rank_type=0&by_time=2&page=1",
		V4_API_URL
	);
	// Audience categories
	let shounen_url = format!("{}/comic/filter/list?cate=3262&size=20&page=1", V4_API_URL);
	let shoujo_url = format!("{}/comic/filter/list?cate=3263&size=20&page=1", V4_API_URL);
	let seinen_url = format!("{}/comic/filter/list?cate=3264&size=20&page=1", V4_API_URL);
	let josei_url = format!("{}/comic/filter/list?cate=13626&size=20&page=1", V4_API_URL);
	let manga_news_url = "https://news.zaimanhua.com/manhuaqingbao";

	let requests = [
		net::get_request(&recommend_url)?, // 0: recommend
		get_api_request(&latest_url)?,     // 1: latest
		get_api_request(&rank_url)?,       // 2: rank
		get_api_request(&shounen_url)?,    // 3: 少年漫画
		get_api_request(&shoujo_url)?,     // 4: 少女漫画
		get_api_request(&seinen_url)?,     // 5: 男青漫画
		get_api_request(&josei_url)?,      // 6: 女青漫画
		net::get_request(manga_news_url)?, // 7: 漫画情报 HTML
	];

	let responses: [core::result::Result<Response, RequestError>; 8] = Request::send_all(requests)
		.try_into()
		.map_err(|_| aidoku::error!("Failed to convert responses"))?;

	let [r0, r1, r2, r3, r4, r5, r6, mut r7] = responses;

	let mut components = Vec::new();

	let mut big_scroller_manga: Vec<Manga> = Vec::new(); // For 109
	let mut banner_links: Vec<aidoku::Link> = Vec::new();

	if let Ok(ref mut resp) = r7
		&& let Ok(html) = resp.get_string()
	{
		banner_links = parse_manga_news_html(&html);
	}

	// Parse recommend/list response - returns raw List, NOT ApiResponse
	if let Ok(resp) = r0
		&& let Ok(categories) = resp.get_json_owned::<Vec<crate::models::RecommendCategory>>()
	{
		for cat in categories {
			let manga_list: Vec<Manga> = cat
				.data
				.iter()
				.filter(|item| item.obj_id > 0)
				.map(|item| Manga {
					key: item.obj_id.to_string(),
					title: item.title.clone(),
					authors: Some(vec![item.sub_title.clone().unwrap_or_default()]),
					cover: Some(item.cover.clone().unwrap_or_default()),
					status: MangaStatus::Unknown,
					..Default::default()
				})
				.collect();

			if manga_list.is_empty() {
				continue;
			}

			// Only handle category 109 (Premium Recommend) as BigScroller
			if cat.category_id == 109 {
				big_scroller_manga = cat.data.iter()
					// Filter only Manga type (1) to avoid Topics/Ads
					.filter(|item| item.obj_id > 0 && item.item_type == 1)
					.map(|item| {
						let mut real_title = item.title.clone();
						let mut manga_cover = item.cover.clone().unwrap_or_default();

						// Fetch details for high-res assets
						// Try to update title/cover from detail API
						if let Ok(req) = Request::get(format!("https://v4api.zaimanhua.com/app/v1/comic/detail/{}", item.obj_id))
							&& let Ok(resp) = req.json_owned::<ApiResponse<DetailData>>()
							&& let Some(detail_root) = resp.data
							&& let Some(detail) = detail_root.data
						{
							if let Some(t) = detail.title {
								real_title = t;
							}

							// We also need to get the cover if available to be safe
							if let Some(c) = detail.cover
								&& !c.is_empty()
							{
								manga_cover = c;
							}
						}

						Manga {
							key: item.obj_id.to_string(),
							title: real_title, // Real Manga Title
							authors: Some(vec![item.sub_title.clone().unwrap_or_default()]), // Author
							// Editorial remark goes to description/subtitle equivalent in BigScroller
							description: Some(item.title.clone()),
							cover: Some(manga_cover),
							status: MangaStatus::Unknown,
							..Default::default()
						}
					})
					.collect();
			}
		}
	}

	let mut latest_entries: Vec<MangaWithChapter> = Vec::new();
	if let Ok(resp) = r1
		&& let Ok(response) =
			resp.get_json_owned::<crate::models::ApiResponse<crate::models::FilterData>>()
		&& let Some(data) = response.data
	{
		latest_entries = data
			.comic_list
			.into_iter()
			.map(|item| item.into_manga_with_chapter())
			.collect();
	}

	fn parse_rank_page(resp: Response) -> Vec<Manga> {
		if let Ok(response) =
			resp.get_json_owned::<crate::models::ApiResponse<Vec<crate::models::RankItem>>>()
			&& let Some(list) = response.data
		{
			return list
				.into_iter()
				.filter(|item| item.comic_id > 0)
				.map(|item| {
					let mut manga: Manga = item.clone().into();
					// Parse 热度
					let num = item.num.unwrap_or(0);
					manga.description = if num >= 10000 {
						Some(format!("热度 {:.1}万", num as f64 / 10000.0))
					} else if num > 0 {
						Some(format!("热度 {}", num))
					} else {
						None
					};
					manga
				})
				.collect();
		}
		Vec::new()
	}

	// 1 page = 10 items
	let mut hot_entries: Vec<Manga> = Vec::new();
	if let Ok(resp) = r2 {
		hot_entries.extend(parse_rank_page(resp));
	}

	components.push(HomeComponent {
		title: None,
		subtitle: None,
		value: aidoku::HomeComponentValue::ImageScroller {
			links: banner_links,
			auto_scroll_interval: Some(5.0), // Auto scroll every 5 seconds
			width: Some(252),
			height: Some(162),
		},
	});

	if !big_scroller_manga.is_empty() {
		components.push(HomeComponent {
			title: Some("精品推荐".into()),
			subtitle: None,
			value: aidoku::HomeComponentValue::BigScroller {
				entries: big_scroller_manga,
				auto_scroll_interval: Some(8.0),
			},
		});
	}

	components.push(HomeComponent {
		title: Some("人气推荐".into()),
		subtitle: None,
		value: aidoku::HomeComponentValue::MangaList {
			ranking: true,
			page_size: Some(2),
			entries: hot_entries
				.into_iter()
				.map(|manga| {
					// Only show author in subtitle
					let subtitle = manga
						.authors
						.as_ref()
						.filter(|a| !a.is_empty())
						.map(|a| a.join(", "));

					aidoku::Link {
						title: manga.title.clone(),
						subtitle,
						image_url: manga.cover.clone(),
						value: Some(aidoku::LinkValue::Manga(manga)),
					}
				})
				.collect(),
			listing: Some(Listing {
				id: "rank-monthly".into(),
				name: "人气推荐".into(),
				kind: ListingKind::default(),
			}),
		},
	});

	components.push(HomeComponent {
		title: Some("最近更新".into()),
		subtitle: None,
		value: aidoku::HomeComponentValue::MangaChapterList {
			page_size: Some(4),
			entries: latest_entries,
			listing: Some(Listing {
				id: "latest".into(),
				name: "更新".into(),
				kind: ListingKind::default(),
			}),
		},
	});

	// Parse audience category scroller
	fn parse_audience_scroller(resp: Response) -> Vec<aidoku::Link> {
		if let Ok(response) =
			resp.get_json_owned::<crate::models::ApiResponse<crate::models::FilterData>>()
			&& let Some(data) = response.data
		{
			return crate::models::manga_list_from_filter(data.comic_list)
				.entries
				.into_iter()
				.map(|manga| aidoku::Link {
					title: manga.title.clone(),
					subtitle: manga.authors.as_ref().and_then(|a| a.first()).cloned(),
					image_url: manga.cover.clone(),
					value: Some(aidoku::LinkValue::Manga(manga)),
				})
				.collect();
		}
		Vec::new()
	}

	let shounen_links = if let Ok(resp) = r3 {
		parse_audience_scroller(resp)
	} else {
		Vec::new()
	};
	components.push(HomeComponent {
		title: Some("少年漫画".into()),
		subtitle: None,
		value: aidoku::HomeComponentValue::Scroller {
			entries: shounen_links,
			listing: Some(Listing {
				id: "shounen".into(),
				name: "少年漫画".into(),
				kind: ListingKind::default(),
			}),
		},
	});

	let shoujo_links = if let Ok(resp) = r4 {
		parse_audience_scroller(resp)
	} else {
		Vec::new()
	};
	components.push(HomeComponent {
		title: Some("少女漫画".into()),
		subtitle: None,
		value: aidoku::HomeComponentValue::Scroller {
			entries: shoujo_links,
			listing: Some(Listing {
				id: "shoujo".into(),
				name: "少女漫画".into(),
				kind: ListingKind::default(),
			}),
		},
	});

	let seinen_links = if let Ok(resp) = r5 {
		parse_audience_scroller(resp)
	} else {
		Vec::new()
	};
	components.push(HomeComponent {
		title: Some("男青漫画".into()),
		subtitle: None,
		value: aidoku::HomeComponentValue::Scroller {
			entries: seinen_links,
			listing: Some(Listing {
				id: "seinen".into(),
				name: "男青漫画".into(),
				kind: ListingKind::default(),
			}),
		},
	});

	let josei_links = if let Ok(resp) = r6 {
		parse_audience_scroller(resp)
	} else {
		Vec::new()
	};
	components.push(HomeComponent {
		title: Some("女青漫画".into()),
		subtitle: None,
		value: aidoku::HomeComponentValue::Scroller {
			entries: josei_links,
			listing: Some(Listing {
				id: "josei".into(),
				name: "女青漫画".into(),
				kind: ListingKind::default(),
			}),
		},
	});

	Ok(HomeLayout { components })
}

/// HTML structure: .briefnews_con_li contains .dec_img img (image) and h3 a (link)
fn parse_manga_news_html(html: &str) -> Vec<aidoku::Link> {
	let mut links = Vec::new();
	let mut seen_ids: Vec<String> = Vec::new();

	// Split by image markers and extract pairs
	for (i, part) in html.split("images.zaimanhua.com/news/article/").enumerate() {
		if i == 0 || links.len() >= 5 {
			continue;
		}

		// Extract article ID from image path (first segment after split)
		let article_id: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();

		if article_id.is_empty() || seen_ids.contains(&article_id) {
			continue;
		}
		seen_ids.push(article_id.clone());

		// Extract full image URL (find the end quote)
		let img_url_part: String = part
			.chars()
			.take_while(|c| *c != '"' && *c != '\'' && *c != ' ')
			.collect();

		let image_url = format!("https://images.zaimanhua.com/news/article/{}", img_url_part);
		let news_url = format!("https://news.zaimanhua.com/article/{}.html", article_id);

		links.push(aidoku::Link {
			title: String::new(),
			subtitle: None,
			image_url: Some(image_url),
			value: Some(aidoku::LinkValue::Url(news_url)),
		});
	}

	links
}
