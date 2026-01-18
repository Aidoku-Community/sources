use crate::net;
use crate::settings;
use aidoku::{
	HomeComponent, HomeLayout, HomePartialResult, Listing, ListingKind, Manga, MangaStatus,
	MangaWithChapter, Result,
	alloc::{Vec, format, string::ToString, vec},
	imports::net::Response,
	imports::std::send_partial_result,
	imports::html::Document,
};

use crate::models::{ApiResponse, DetailData};

// === Public API ===

/// Build the home page layout
pub fn get_home_layout() -> Result<HomeLayout> {
	// Silent background updates (check-in + cache refresh)
	if let Some(token) = settings::get_token() {
		net::perform_silent_updates(&token);
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

	// Build parallel requests
	let token = settings::get_current_token();
	let token_ref = token.as_deref();

	let recommend_url = net::urls::recommend();
	let latest_url = net::urls::filter_latest_sized(1, 20);
	let rank_url = net::urls::rank(2, 1);
	let shounen_url = net::urls::filter_cate(3262, 1, 20);
	let shoujo_url = net::urls::filter_cate(3263, 1, 20);
	let seinen_url = net::urls::filter_cate(3264, 1, 20);
	let josei_url = net::urls::filter_cate(13626, 1, 20);

	let mut batch = net::RequestBatch::new();

	// 0: banner
	let manga_news_url = net::urls::manga_news();
	let slot_banner = batch.add_unless_blocked(&manga_news_url);

	// 1-7: Standard requests
	let slot_recommend = batch.get(&recommend_url)?;
	let slot_latest = batch.auth(&latest_url, token_ref)?;
	let slot_rank = batch.auth(&rank_url, token_ref)?;
	let slot_shounen = batch.auth(&shounen_url, token_ref)?;
	let slot_shoujo = batch.auth(&shoujo_url, token_ref)?;
	let slot_seinen = batch.auth(&seinen_url, token_ref)?;
	let slot_josei = batch.auth(&josei_url, token_ref)?;

	let mut responses = batch.send_all();

	let resp_recommend = responses[slot_recommend].take();
	let resp_latest = responses[slot_latest].take();
	let resp_rank = responses[slot_rank].take();
	let resp_shounen = responses[slot_shounen].take();
	let resp_shoujo = responses[slot_shoujo].take();
	let resp_seinen = responses[slot_seinen].take();
	let resp_josei = responses[slot_josei].take();

	let mut components = Vec::new();

	let mut big_scroller_manga: Vec<Manga> = Vec::new();
	let mut banner_links: Vec<aidoku::Link> = Vec::new();

	// Banner: Fetch separately only when NOT using proxy (news.zaimanhua.com has DNSSEC issues)
	// Banner Processing
	if let Some(resp) = responses[slot_banner].take()
		&& let Ok(doc) = resp.get_html()
	{
		banner_links = parse_manga_news_doc(doc);
	}

	// Parse recommend/list response - returns raw List, NOT ApiResponse
	if let Some(resp) = resp_recommend
		&& let Ok(categories) = resp.get_json_owned::<Vec<crate::models::RecommendCategory>>()
	{
		for cat in categories {
			// Only handle category 109 (Premium Recommend) as BigScroller
			if cat.category_id != 109 || cat.data.is_empty() {
				continue;
			}

			big_scroller_manga = cat.data.into_iter()
				// Filter only Manga type (1)
				.filter(|item| item.obj_id > 0 && item.item_type == 1)
				.map(|item| {
					let mut real_title = item.title.clone();
					let mut manga_cover = item.cover.clone().unwrap_or_default();

					// Fetch details for high-res assets
					if let Ok(req) = net::get_request(&net::urls::detail(item.obj_id))
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
	if let Some(resp) = resp_latest
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

	// 1 page = 10 items
	let mut hot_entries: Vec<Manga> = Vec::new();
	if let Some(resp) = resp_rank {
		hot_entries.extend(parse_rank_response(resp));
	}

	// Only show banner if links exist (proxy mode skips this)
	if !banner_links.is_empty() {
		components.push(HomeComponent {
			title: None,
			subtitle: None,
			value: aidoku::HomeComponentValue::ImageScroller {
				links: banner_links,
				auto_scroll_interval: None,
				width: Some(252),
				height: Some(162),
			},
		});
	}

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

	let shounen_links = resp_shounen
		.map(parse_filter_response)
		.unwrap_or_default();
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

	let shoujo_links = resp_shoujo
		.map(parse_filter_response)
		.unwrap_or_default();
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

	let seinen_links = resp_seinen
		.map(parse_filter_response)
		.unwrap_or_default();
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

	let josei_links = resp_josei
		.map(parse_filter_response)
		.unwrap_or_default();
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

// === Private Helpers ===

/// Parse rank API response into Manga list
fn parse_rank_response(resp: Response) -> Vec<Manga> {
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

/// Parse filter API response into Link list (for audience scrollers)
fn parse_filter_response(resp: Response) -> Vec<aidoku::Link> {
	if let Ok(response) =
		resp.get_json_owned::<crate::models::ApiResponse<crate::models::FilterData>>()
		&& let Some(data) = response.data
	{
		return data.comic_list.into_iter().map(Into::into).collect();
	}
	Vec::new()
}

/// Parse manga news HTML document into Link list
/// HTML structure: .briefnews_con_li contains .dec_img img (image) and h3 a (link)
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

			let final_image_url = if image_url.starts_with("http") {
				image_url
			} else {
				format!("{}{}", crate::NEWS_URL, image_url)
			};

			let full_url = if url.starts_with("http") {
				url
			} else {
				format!("{}{}", crate::NEWS_URL, url)
			};

			links.push(aidoku::Link {
				title,
				subtitle: None,
				image_url: Some(final_image_url),
				value: Some(aidoku::LinkValue::Url(full_url)),
			});
		}
	}

	links
}
