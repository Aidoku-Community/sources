use aidoku::{
	Chapter, ContentRating, DeepLinkResult, FilterValue, Manga, MangaPageResult, MangaStatus, Page,
	PageContent, Result, Viewer,
	alloc::{String, Vec, format, string::ToString, vec},
	bail,
	helpers::uri::encode_uri_component,
	imports::{net::Request, std::send_partial_result},
};
use serde::Deserialize;

use crate::helper::*;

#[derive(Deserialize)]
struct ApiArticleInfo {
	#[serde(rename = "titleName")]
	title_name: Option<String>,
	#[serde(rename = "posterThumbnailUrl")]
	poster_thumbnail_url: Option<String>,
	#[serde(rename = "thumbnailUrl")]
	thumbnail_url: Option<String>,
	synopsis: Option<String>,
	#[serde(rename = "communityArtists")]
	community_artists: Option<Vec<ApiCommunityArtist>>,
	finished: Option<bool>,
	rest: Option<bool>,
	#[serde(rename = "curationTagList")]
	curation_tag_list: Option<Vec<ApiTagInfo>>,
}

#[derive(Deserialize)]
struct ApiCommunityArtist {
	name: Option<String>,
}

#[derive(Deserialize)]
struct ApiTagInfo {
	#[serde(rename = "tagName")]
	tag_name: Option<String>,
}

#[derive(Deserialize)]
struct ApiSearchResponse {
	#[serde(rename = "searchList")]
	search_list: Option<Vec<ApiSearchItem>>,
	#[serde(rename = "pageInfo")]
	page_info: Option<ApiSearchPageInfo>,
}

#[derive(Deserialize)]
struct ApiSearchPageInfo {
	#[serde(rename = "nextPage")]
	next_page: Option<i32>,
}

#[derive(Deserialize)]
struct ApiSearchItem {
	#[serde(rename = "titleId")]
	title_id: i64,
	#[serde(rename = "titleName")]
	title_name: Option<String>,
	#[serde(rename = "thumbnailUrl")]
	thumbnail_url: Option<String>,
	#[serde(rename = "displayAuthor")]
	display_author: Option<String>,
	#[serde(rename = "webtoonLevelCode")]
	webtoon_level_code: Option<String>,
	synopsis: Option<String>,
}

/// Helper to parse manga card elements from specific list selectors
fn parse_manga_items(html: &aidoku::imports::html::Document, selector: &str) -> Vec<Manga> {
	let mut mangas: Vec<Manga> = Vec::new();
	let Some(items) = html.select(selector) else {
		return mangas;
	};

	for node in items {
		// Skip advertisement banners
		if node.has_class("banner") {
			continue;
		}

		let href = if let Some(link) = node.select_first("a[href*=\"titleId=\"]") {
			link.attr("href").unwrap_or_default()
		} else if let Some(h) = node.attr("href").filter(|h| h.contains("titleId=")) {
			h
		} else {
			continue;
		};

		let Some(id) = get_title_id(&href) else {
			continue;
		};

		let cover = node.select_first("img").and_then(|e| e.attr("src"));

		// If entry already exists, upgrade low-res cover (IMAG19) and preserve ranking order
		if let Some(existing) = mangas.iter_mut().find(|m| m.key == id) {
			if let Some(ref c) = cover
				&& !c.contains("IMAG19")
			{
				existing.cover = cover;
			}
			continue;
		}

		let mut title = node
			.select_first(".title_text")
			.and_then(|e| e.text())
			.unwrap_or_default();
		if title.trim().is_empty() {
			title = node
				.select_first("strong, .title")
				.and_then(|e| e.text())
				.unwrap_or_default();
		}
		let title = String::from(title.trim());
		if title.is_empty() {
			continue;
		}

		let authors = node
			.select_first(".author")
			.and_then(|e| e.text())
			.map(|a| split_authors(&a));
		let artists = authors
			.as_ref()
			.map(|list| list.iter().map(|s| String::from(s.as_str())).collect());

		let full_url = if href.starts_with("http") {
			href
		} else {
			format!("{BASE_URL}{href}")
		};

		mangas.push(Manga {
			key: id,
			cover,
			title,
			authors,
			artists,
			url: Some(full_url),
			viewer: Viewer::Webtoon,
			..Default::default()
		});
	}
	mangas
}

/// Helper to parse weekday list with sorting
fn parse_weekday_list(week: &str, sort: &str) -> Result<MangaPageResult> {
	let url = if week.is_empty() {
		format!("{BASE_URL}/webtoon/weekday?sort={sort}")
	} else {
		format!("{BASE_URL}/webtoon/weekday?week={week}&sort={sort}")
	};
	let html = request(&url)?.html()?;
	let entries = parse_manga_items(&html, "ul.list_toon > li.item");

	Ok(MangaPageResult {
		entries,
		has_next_page: false,
	})
}

/// Helper to parse finished webtoons with pagination
fn parse_finish_list(page: i32, sort: &str) -> Result<MangaPageResult> {
	let url = format!("{BASE_URL}/webtoon/finish?sort={sort}&genre=&page={page}");
	let html = request(&url)?.html()?;
	let entries = parse_manga_items(&html, "ul.list_toon > li.item");

	let next_btn = html.select_first("a.btn_next");
	let has_next_page = if let Some(btn) = next_btn {
		let href = btn.attr("href").unwrap_or_default();
		!entries.is_empty() && !btn.has_class("disabled") && !href.is_empty() && href != "#"
	} else {
		false
	};

	Ok(MangaPageResult {
		entries,
		has_next_page,
	})
}

/// Helper to parse Best Challenge series with pagination
fn parse_best_challenge_list(page: i32, sort: &str) -> Result<MangaPageResult> {
	let url = format!("{BASE_URL}/bestChallenge/genre?sort={sort}&genre=&page={page}");
	let html = request(&url)?.html()?;
	let entries = parse_manga_items(&html, "ul.list_toon_wide > li.item");

	let next_btn = html.select_first("a.btn_next");
	let has_next_page = if let Some(btn) = next_btn {
		let href = btn.attr("href").unwrap_or_default();
		!entries.is_empty() && !btn.has_class("disabled") && !href.is_empty() && href != "#"
	} else {
		false
	};

	Ok(MangaPageResult {
		entries,
		has_next_page,
	})
}

/// Search manga by query or browse with filters
pub fn parse_search_manga_list(
	query: Option<String>,
	page: i32,
	filters: Vec<FilterValue>,
) -> Result<MangaPageResult> {
	let mut search_type = "ALL";
	let mut weekday = "";
	let mut sort_index: i32 = 0;
	let mut search_query = query;

	for filter in &filters {
		match filter {
			FilterValue::Select { id, value } if id == "searchType" && !value.is_empty() => {
				search_type = value.as_str();
			}
			FilterValue::Select { id, value } if id == "weekday" => {
				weekday = value.as_str();
			}
			FilterValue::Sort { id, index, .. } if id == "sort" => {
				sort_index = *index;
			}
			FilterValue::Text { id, value } if id == "author" && !value.is_empty() => {
				if search_query.is_none() {
					search_query = Some(value.clone());
				}
			}
			_ => {}
		}
	}

	let webtoon_sort = match sort_index {
		0 => "ALL_READER",
		1 => "FEMALE_READER",
		2 => "MALE_READER",
		3 => "HIT",
		4 => "UPDATE",
		_ => "ALL_READER",
	};

	let best_challenge_sort = match sort_index {
		4 => "UPDATE",
		5 => "STAR",
		_ => "HIT",
	};

	if let Some(ref q) = search_query {
		let trimmed = q.trim();
		if !trimmed.is_empty() {
			let encoded = encode_uri_component(trimmed);

			// Try official JSON search API first for fast pagination
			let api_url = if search_type == "BEST_CHALLENGE" {
				format!(
					"https://comic.naver.com/api/search/bestChallenge?keyword={encoded}&page={page}"
				)
			} else {
				format!("https://comic.naver.com/api/search/webtoon?keyword={encoded}&page={page}")
			};

			if let Ok(resp) = request(&api_url).and_then(|r| r.json_owned::<ApiSearchResponse>()) {
				let has_next_page = resp.page_info.and_then(|p| p.next_page).unwrap_or(0) != 0;
				let entries = resp
					.search_list
					.unwrap_or_default()
					.into_iter()
					.map(|item| {
						let is_best = item.webtoon_level_code.as_deref() == Some("BEST_CHALLENGE")
							|| search_type == "BEST_CHALLENGE";
						let key = if is_best {
							format!("{}-best", item.title_id)
						} else {
							item.title_id.to_string()
						};
						let url = Some(get_manga_url(&key));
						let authors = item.display_author.map(|a| split_authors(&a));
						let artists = authors
							.as_ref()
							.map(|list| list.iter().map(|s| String::from(s.as_str())).collect());

						Manga {
							key,
							title: item.title_name.unwrap_or_default(),
							cover: item.thumbnail_url,
							url,
							description: item.synopsis,
							authors,
							artists,
							viewer: Viewer::Webtoon,
							..Default::default()
						}
					})
					.collect();

				return Ok(MangaPageResult {
					entries,
					has_next_page,
				});
			}

			// HTML search fallback
			let url = if search_type == "ALL" {
				format!("{BASE_URL}/search/result?keyword={encoded}")
			} else {
				format!("{BASE_URL}/search/result?keyword={encoded}&searchType={search_type}")
			};
			let html = request(&url)?.html()?;
			let entries = parse_manga_items(&html, "ul.toon_lst > li");

			return Ok(MangaPageResult {
				entries,
				has_next_page: false,
			});
		}
	}

	// Filter-based browse (no search query)
	if search_type == "BEST_CHALLENGE" {
		parse_best_challenge_list(page, best_challenge_sort)
	} else if weekday == "finish" {
		parse_finish_list(page, webtoon_sort)
	} else {
		if page > 1 {
			return Ok(MangaPageResult {
				entries: Vec::new(),
				has_next_page: false,
			});
		}
		parse_weekday_list(weekday, webtoon_sort)
	}
}

/// Parses webtoon details (title, cover, author, description, status, genre, 19+ check)
pub fn parse_manga_details(mut manga: Manga) -> Result<Manga> {
	let url = get_manga_url(&manga.key);
	let html = request(&url)?.html()?;

	let is_login_page = html
		.select_first("title")
		.and_then(|t| t.text())
		.is_some_and(|s| s.contains("로그인"))
		|| html.select_first("form#frmNIDLogin").is_some();

	if is_login_page {
		manga.content_rating = ContentRating::NSFW;
		manga.viewer = Viewer::Webtoon;
		manga.url = Some(url);

		// Fallback to official API for metadata even when not logged in
		let clean_id = manga.key.strip_suffix("-best").unwrap_or(&manga.key);
		let api_url = format!("https://comic.naver.com/api/article/list/info?titleId={clean_id}");
		if let Ok(info) = request(&api_url).and_then(|r| r.json_owned::<ApiArticleInfo>()) {
			if let Some(name) = info.title_name {
				manga.title = name;
			}
			if let Some(cover) = info.poster_thumbnail_url.or(info.thumbnail_url) {
				manga.cover = Some(cover);
			}
			if let Some(desc) = info.synopsis {
				manga.description = Some(desc);
			}
			if let Some(artists) = info.community_artists {
				let authors: Vec<String> = artists
					.into_iter()
					.filter_map(|a| a.name)
					.map(|s| String::from(s.trim()))
					.filter(|s| !s.is_empty())
					.collect();
				if !authors.is_empty() {
					let artists = authors.iter().map(|s| String::from(s.as_str())).collect();
					manga.artists = Some(artists);
					manga.authors = Some(authors);
				}
			}
			if let Some(tags) = info.curation_tag_list {
				let tag_names: Vec<String> = tags
					.into_iter()
					.filter_map(|t| t.tag_name)
					.filter(|s| !s.trim().is_empty())
					.collect();
				if !tag_names.is_empty() {
					manga.tags = Some(tag_names);
				}
			}
			manga.status = if info.finished.unwrap_or(false) {
				MangaStatus::Completed
			} else if info.rest.unwrap_or(false) {
				MangaStatus::Hiatus
			} else {
				MangaStatus::Ongoing
			};
		}

		if manga.description.is_none() {
			manga.description = Some(String::from(
				"로그인이 필요한 작품입니다. 소스 설정에서 네이버 로그인을 완료해주세요.",
			));
		}
		if let Some(ref mut tags) = manga.tags {
			if !tags.iter().any(|t| t == "성인") {
				tags.push(String::from("성인"));
			}
		} else {
			manga.tags = Some(vec![String::from("성인")]);
		}

		return Ok(manga);
	}

	let title = html
		.select_first("meta[property=\"og:title\"]")
		.and_then(|e| e.attr("content"))
		.or_else(|| {
			html.select_first(".info_area .title, .title_area strong")
				.and_then(|e| e.text())
		});
	if let Some(t) = title {
		manga.title = t;
	}

	let cover = html
		.select_first("meta[property=\"og:image\"]")
		.and_then(|e| e.attr("content"));
	if cover.is_some() {
		manga.cover = cover;
	}

	let description = html
		.select_first("meta[property=\"og:description\"]")
		.and_then(|e| e.attr("content"))
		.or_else(|| html.select_first(".summary, .desc").and_then(|e| e.text()));
	if description.is_some() {
		manga.description = description;
	}

	let author = html
		.select_first(".author, .info_area .author, .writer, .info .author_area")
		.and_then(|e| e.text());
	if let Some(a) = author {
		let authors = split_authors(&a);
		if !authors.is_empty() {
			let artists = authors.iter().map(|s| String::from(s.as_str())).collect();
			manga.artists = Some(artists);
			manga.authors = Some(authors);
		}
	}

	let mut tags: Vec<String> = Vec::new();
	if let Some(genre_items) = html.select(".genre dd span, .genre dd li, .tag_item") {
		for node in genre_items {
			if let Some(g_text) = node.text() {
				let trimmed = g_text.trim();
				if !trimmed.is_empty() && !tags.iter().any(|t| t == trimmed) {
					tags.push(String::from(trimmed));
				}
			}
		}
	}

	let is_adult = tags.iter().any(|t| t.contains("19") || t.contains("성인"));
	manga.content_rating = if is_adult {
		ContentRating::NSFW
	} else {
		ContentRating::Safe
	};
	if !tags.is_empty() {
		manga.tags = Some(tags);
	}

	let status_text = html
		.select_first(".week_day .list_detail, .detail .week_day")
		.and_then(|e| e.text())
		.unwrap_or_default();
	let is_hiatus = html
		.select_first("span.bullet.break, span.bullet_break")
		.is_some()
		|| html
			.select_first(".area_info, .section_toon_info")
			.and_then(|e| e.text())
			.is_some_and(|t| t.contains("휴재"))
		|| status_text.contains("휴재");
	let is_completed = status_text.contains("완결")
		|| html
			.select_first(".week_day")
			.and_then(|e| e.text())
			.is_some_and(|t| t.contains("완결"));
	manga.status = if is_completed {
		MangaStatus::Completed
	} else if is_hiatus {
		MangaStatus::Hiatus
	} else {
		MangaStatus::Ongoing
	};

	manga.url = Some(url);
	manga.viewer = Viewer::Webtoon;

	Ok(manga)
}

/// Parses all chapters for a given manga by paginating through list pages
pub fn parse_chapter_list(manga_id: &str) -> Result<Vec<Chapter>> {
	let mut chapters: Vec<Chapter> = Vec::new();
	let mut page = 1;

	loop {
		let url = if let Some(clean_id) = manga_id.strip_suffix("-best") {
			format!("{BASE_URL}/bestChallenge/list?titleId={clean_id}&sortOrder=DESC&page={page}")
		} else {
			format!("{BASE_URL}/webtoon/list?titleId={manga_id}&sortOrder=DESC&page={page}")
		};

		let req = request(&url)?;
		let html = req.html()?;

		// Check for 19+ adult login redirect
		let is_login_page = html
			.select_first("title")
			.and_then(|t| t.text())
			.is_some_and(|s| s.contains("로그인"))
			|| html.select_first("form#frmNIDLogin").is_some()
			|| html.select_first("input[name='dynamicKey']").is_some();

		if is_login_page {
			bail!("로그인이 필요한 작품입니다. 소스 설정에서 네이버 로그인을 완료해주세요.");
		}

		let ep_items = match html.select("ul.section_episode_list li.item") {
			Some(items) if !items.is_empty() => items,
			_ => match html.select("li.item[data-no]") {
				Some(items) if !items.is_empty() => items,
				_ => match html.select("a[href*=\"detail?\"]") {
					Some(items) => items,
					None => break,
				},
			},
		};

		let mut found_new = false;
		for node in ep_items {
			let href = node
				.attr("href")
				.or_else(|| node.select_first("a").and_then(|a| a.attr("href")))
				.unwrap_or_default();

			let chapter_id = node
				.attr("data-no")
				.filter(|no| !no.is_empty())
				.or_else(|| get_chapter_id(&href));
			let Some(chapter_id) = chapter_id else {
				continue;
			};
			if chapters.iter().any(|c| c.key == chapter_id) {
				continue;
			}
			found_new = true;

			let is_locked = node.has_class("lock")
				|| node
					.select_first(".ico_comic .blind")
					.and_then(|e| e.text())
					.map(|t| t.contains("유료"))
					.unwrap_or(false)
				|| node
					.select_first("em.cookie_txt, .cookie_txt, .ico_cookie")
					.is_some() || href == "#"
				|| href.is_empty();

			let mut raw_title = node
				.select_first(".name")
				.and_then(|e| e.text())
				.unwrap_or_default();
			if raw_title.trim().is_empty() {
				raw_title = node
					.select_first("strong.title, .title")
					.and_then(|e| e.text())
					.unwrap_or_default();
			}
			let raw_title = if raw_title.trim().is_empty() {
				format!("{chapter_id}화")
			} else {
				String::from(raw_title.trim())
			};

			let fallback_no = chapter_id.parse::<f32>().unwrap_or(-1.0);
			let chapter_num = extract_chapter_number(&raw_title, fallback_no);
			let volume_number = extract_volume_number(&raw_title);
			let title = raw_title;
			let date_text = node
				.select_first(".date")
				.and_then(|e| e.text())
				.unwrap_or_default();
			let date_uploaded = parse_korean_date(&date_text);

			let thumbnail = node
				.select_first("div.thumbnail img, .item_thumb img")
				.and_then(|img| img.attr("src"));

			let full_chapter_url = if href.contains("detail?") {
				if href.starts_with("http") {
					href
				} else {
					format!("{BASE_URL}{href}")
				}
			} else {
				get_chapter_url(&chapter_id, manga_id)
			};

			chapters.push(Chapter {
				key: chapter_id,
				title: Some(title),
				volume_number,
				chapter_number: (chapter_num >= 0.0).then_some(chapter_num),
				date_uploaded,
				url: Some(full_chapter_url),
				language: Some(String::from("ko")),
				thumbnail,
				locked: is_locked,
				..Default::default()
			});
		}

		if !found_new {
			break;
		}

		// Check pagination button
		let next_btn = html.select_first("a.btn_next");
		if let Some(btn) = next_btn {
			let next_href = btn.attr("href").unwrap_or_default();
			if btn.has_class("disabled") || next_href.is_empty() || next_href == "#" {
				break;
			}
		} else {
			break;
		}

		page += 1;
		if page > 500 {
			break;
		}
	}

	Ok(chapters)
}

/// Unified manga update function (details + chapters)
pub fn parse_manga_update(
	mut manga: Manga,
	needs_details: bool,
	needs_chapters: bool,
) -> Result<Manga> {
	if needs_details {
		manga = parse_manga_details(manga)?;
		if needs_chapters {
			send_partial_result(&manga);
		}
	}
	if needs_chapters {
		let chapters = parse_chapter_list(&manga.key)?;
		manga.chapters = Some(chapters);
	}
	Ok(manga)
}

/// Parses all cut images of a specific episode
pub fn parse_page_list(manga_id: &str, chapter_id: &str) -> Result<Vec<Page>> {
	let url = get_chapter_url(chapter_id, manga_id);
	let html = request(&url)?.html()?;

	let is_login_page = html
		.select_first("title")
		.and_then(|t| t.text())
		.is_some_and(|s| s.contains("로그인"))
		|| html.select_first("form#frmNIDLogin").is_some();

	if is_login_page {
		bail!("로그인이 필요한 작품입니다. 소스 설정에서 네이버 로그인을 완료해주세요.");
	}

	let clean_id = manga_id.strip_suffix("-best").unwrap_or(manga_id);
	let mut candidate_urls: Vec<String> = Vec::new();
	let mut img_nodes = html.select("img.toon_image");
	if img_nodes.as_ref().is_none_or(|l| l.is_empty()) {
		img_nodes = html.select("img.swiper-lazy");
	}
	if img_nodes.as_ref().is_none_or(|l| l.is_empty()) {
		img_nodes = html.select(
			"div.cut_flick_wrap img, div#mflick img, div.wt_viewer img, #toon_layer img, div.toon_view_area img, div.viewer_img img, .swiper-slide img",
		);
	}

	if let Some(items) = img_nodes {
		for node in items {
			let mut img_url = node.attr("data-src").unwrap_or_default();
			if img_url.is_empty() || img_url.contains("bg_transparency.png") {
				img_url = node.attr("src").unwrap_or_default();
			}
			if img_url.is_empty()
				|| img_url.contains("bg_transparency.png")
				|| img_url.contains("agerate")
				|| img_url.contains("age_")
				|| img_url.contains("kw-wcommunity-phinf")
				|| img_url.contains("naverwebtoon-phinf")
				|| img_url.contains("thumbnail")
				|| img_url.contains("static/")
				|| img_url.contains("upload_")
				|| img_url.contains("p100")
			{
				continue;
			}

			// Legitimate comic cut images must come from comic image CDN
			if !img_url.contains("image-comic.pstatic.net") {
				continue;
			}

			if candidate_urls.iter().any(|u| u == &img_url) {
				continue;
			}

			candidate_urls.push(img_url);
		}
	}

	let episode_path = format!("/{clean_id}/{chapter_id}/");
	let has_path_matches = candidate_urls.iter().any(|u| u.contains(&episode_path));

	let pages: Vec<Page> = candidate_urls
		.into_iter()
		.filter(|u| !has_path_matches || u.contains(&episode_path))
		.map(|u| Page {
			content: PageContent::url(u),
			..Default::default()
		})
		.collect();

	if pages.is_empty() {
		let raw_html = html
			.select_first("body")
			.and_then(|b| b.html())
			.unwrap_or_default();
		if raw_html.contains("유료로 전환된 회차")
			|| raw_html.contains("구매")
			|| raw_html.contains("대여")
			|| raw_html.contains("cookie_txt")
		{
			bail!("구매 또는 대여가 필요한 유료 회차입니다.");
		}
		bail!("회차 이미지를 불러올 수 없습니다.");
	}

	Ok(pages)
}

/// Handles image request modification (injected Referer + User-Agent)
pub fn get_image_request(url: String) -> Result<Request> {
	request(&url)
}

/// Handles deep linking for comic.naver.com URLs
pub fn parse_deep_link(url: &str) -> Result<Option<DeepLinkResult>> {
	let Some(manga_key) = get_title_id(url) else {
		return Ok(None);
	};
	if let Some(chapter_id) = get_chapter_id(url) {
		Ok(Some(DeepLinkResult::Chapter {
			manga_key,
			key: chapter_id,
		}))
	} else {
		Ok(Some(DeepLinkResult::Manga { key: manga_key }))
	}
}
