use crate::net;
use crate::settings;
use crate::V4_API_URL;
use aidoku::{
	HomeComponent, HomeLayout, HomePartialResult, Listing, ListingKind, Manga, MangaStatus,
	MangaWithChapter, Result,
	alloc::{Vec, format, string::ToString, vec},
	imports::net::RequestError,
	imports::net::{Request, Response},
	imports::std::send_partial_result,
	imports::html::Document,
};

use crate::models::{ApiResponse, DetailData};

/// Build the home page layout
pub fn get_home_layout() -> Result<HomeLayout> {
	// Auto check-in when entering home (moved from Source::new to avoid blocking init)
	if let Some(token) = settings::get_token()
		&& settings::get_auto_checkin()
		&& !settings::has_checkin_flag()
		&& let Ok(true) = net::check_in(&token)
	{
		settings::set_last_checkin();
	}

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

	// Build parallel requests using inline URLs
	let manga_news_url = "https://news.zaimanhua.com/manhuaqingbao";
	let token = settings::get_current_token();
	let token_ref = token.as_deref();

	let recommend_url = format!("{}/comic/recommend/list", V4_API_URL);
	let latest_url = format!("{}/comic/filter/list?sortType=1&page=1&size=20", V4_API_URL);
	let rank_url = format!("{}/comic/rank/list?rank_type=0&by_time=2&page=1", V4_API_URL);
	let shounen_url = format!("{}/comic/filter/list?cate=3262&size=20&page=1", V4_API_URL);
	let shoujo_url = format!("{}/comic/filter/list?cate=3263&size=20&page=1", V4_API_URL);
	let seinen_url = format!("{}/comic/filter/list?cate=3264&size=20&page=1", V4_API_URL);
	let josei_url = format!("{}/comic/filter/list?cate=13626&size=20&page=1", V4_API_URL);

	let requests = [
		net::get_request(&recommend_url)?,   // 0: recommend (no auth needed)
		net::auth_request(&latest_url, token_ref)?,                              // 1: latest
		net::auth_request(&rank_url, token_ref)?,                                // 2: rank
		net::auth_request(&shounen_url, token_ref)?,                             // 3: 少年漫画
		net::auth_request(&shoujo_url, token_ref)?,                              // 4: 少女漫画
		net::auth_request(&seinen_url, token_ref)?,                              // 5: 男青漫画
		net::auth_request(&josei_url, token_ref)?,                               // 6: 女青漫画
		net::get_request(manga_news_url)?,  // 7: 漫画情报 HTML
	];

	let responses: [core::result::Result<Response, RequestError>; 8] = Request::send_all(requests)
		.try_into()
		.map_err(|_| aidoku::error!("Response conversion failed"))?;

	let [resp_recommend, resp_latest, resp_rank, resp_shounen, resp_shoujo, resp_seinen, resp_josei, resp_news] = responses;

	let mut components = Vec::new();

	let mut big_scroller_manga: Vec<Manga> = Vec::new(); // For 109
	let mut banner_links: Vec<aidoku::Link> = Vec::new();

	if let Ok(resp) = resp_news
		&& let Ok(doc) = resp.get_html()
	{
		banner_links = parse_manga_news_doc(doc);
	}

	// Parse recommend/list response - returns raw List, NOT ApiResponse
	if let Ok(resp) = resp_recommend
		&& let Ok(categories) = resp.get_json_owned::<Vec<crate::models::RecommendCategory>>()
	{
		for cat in categories {
			// Only handle category 109 (Premium Recommend) as BigScroller
			if cat.category_id != 109 || cat.data.is_empty() {
				continue;
			}

			big_scroller_manga = cat.data.into_iter()
				// Filter only Manga type (1) to avoid Topics/Ads
				.filter(|item| item.obj_id > 0 && item.item_type == 1)
				.map(|item| {
					let mut real_title = item.title.clone();
					let mut manga_cover = item.cover.clone().unwrap_or_default();

					// Fetch details for high-res assets
					if let Ok(req) = net::get_request(&format!("{}/comic/detail/{}", V4_API_URL, item.obj_id))
						&& let Ok(resp) = req.json_owned::<ApiResponse<DetailData>>()
						&& let Some(detail_root) = resp.data
						&& let Some(detail) = detail_root.data
					{
						if let Some(t) = detail.title {
							real_title = t;
						}

						if let Some(c) = detail.cover
							&& !c.is_empty()
						{
							manga_cover = c;
						}
					}

					Manga {
						key: item.obj_id.to_string(),
						title: real_title,
						authors: Some(vec![item.sub_title.unwrap_or_default()]),
						description: Some(item.title),
						cover: Some(manga_cover),
						status: MangaStatus::Unknown,
						..Default::default()
					}
				})
				.collect();
		}
	}

	let mut latest_entries: Vec<MangaWithChapter> = Vec::new();
	if let Ok(resp) = resp_latest
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
				.map(Into::into)
				.collect();
		}
		Vec::new()
	}

	// 1 page = 10 items
	let mut hot_entries: Vec<Manga> = Vec::new();
	if let Ok(resp) = resp_rank {
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
			return data.comic_list.into_iter().map(Into::into).collect();
		}
		Vec::new()
	}

	let shounen_links = if let Ok(resp) = resp_shounen {
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

	let shoujo_links = if let Ok(resp) = resp_shoujo {
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

	let seinen_links = if let Ok(resp) = resp_seinen {
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

	let josei_links = if let Ok(resp) = resp_josei {
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
/// Used for parsing the news section which is not available via JSON API
fn parse_manga_news_doc(doc: Document) -> Vec<aidoku::Link> {
	let mut links = Vec::new();

	// Use generic class selector (div or li)
	if let Some(list) = doc.select(".briefnews_con_li") {
		for el in list {
			if links.len() >= 5 {
				break;
			}

			let Some(img_node) = el.select_first(".dec_img img") else { continue };
			let Some(image_url) = img_node.attr("src") else { continue };

			let Some(link_node) = el.select_first("h3 a") else { continue };
			let Some(title) = link_node.text() else { continue };
			let Some(url) = link_node.attr("href") else { continue };

			if image_url.is_empty() || url.is_empty() {
				continue;
			}

			let full_url = if url.starts_with("http") {
				url
			} else {
				format!("{}{}", net::NEWS_URL, url)
			};

			links.push(aidoku::Link {
				title,
				subtitle: None,
				image_url: Some(image_url),
				value: Some(aidoku::LinkValue::Url(full_url)),
			});
		}
	}

	links
}
