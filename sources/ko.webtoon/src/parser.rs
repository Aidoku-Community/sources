use aidoku::{
	Chapter, ContentRating, DeepLinkResult, FilterValue, HomeComponent, HomeComponentValue,
	HomeLayout, Link, Listing, Manga, MangaPageResult, MangaStatus, Page, PageContent, Viewer,
	alloc::{String, Vec, format, vec},
	helpers::uri::encode_uri_component,
	imports::{
		error::{AidokuError, Result},
		net::Request,
		std::send_partial_result,
	},
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
	#[serde(rename = "displayAuthor")]
	display_author: Option<String>,
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

/// Helper to parse manga card elements from list / weekday / finish pages
fn parse_manga_cards(html: &aidoku::imports::html::Document) -> Vec<Manga> {
	let mut mangas: Vec<Manga> = Vec::new();
	let selector = "a[href*=\"/webtoon/list?titleId=\"], a[href*=\"/bestChallenge/list?titleId=\"]";
	if let Some(items) = html.select(selector) {
		for node in items {
			let href = node.attr("href").unwrap_or_default();
			let id = get_title_id(&href);
			if id.is_empty() {
				continue;
			}

			let cover = node.select_first("img").and_then(|e| e.attr("src"));

			// If entry already exists, upgrade low-res cover (IMAG19)
			if let Some(existing) = mangas.iter_mut().find(|m| m.key == id) {
				if let Some(ref c) = cover
					&& !c.contains("IMAG19")
				{
					existing.cover = cover;
				}
				continue;
			}

			let mut title = node
				.select_first("strong")
				.and_then(|e| e.text())
				.unwrap_or_default();
			if title.trim().is_empty() {
				title = node
					.select_first(".title")
					.and_then(|e| e.text())
					.unwrap_or_default();
			}
			let title = String::from(title.trim());

			let full_url = if href.starts_with("http") {
				href
			} else {
				format!("{BASE_URL}{href}")
			};

			mangas.push(Manga {
				key: id,
				cover,
				title,
				url: Some(full_url),
				viewer: Viewer::Webtoon,
				..Default::default()
			});
		}
	}
	mangas
}

/// Helper to parse weekday list (mon, tue, wed, thu, fri, sat, sun)
fn parse_weekday_list(week: &str, page: i32) -> Result<MangaPageResult> {
	if page > 1 {
		return Ok(MangaPageResult::default());
	}
	let url = format!("{BASE_URL}/webtoon/weekday?week={week}");
	let html = request(&url)?.html()?;
	let entries = parse_manga_cards(&html);

	Ok(MangaPageResult {
		entries,
		has_next_page: false,
	})
}

/// Helper to parse finished webtoons with pagination
fn parse_finish_list(page: i32, sort: &str) -> Result<MangaPageResult> {
	let url = format!("{BASE_URL}/webtoon/finish?page={page}&sort={sort}");
	let html = request(&url)?.html()?;
	let entries = parse_manga_cards(&html);

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
fn parse_best_challenge_list(page: i32) -> Result<MangaPageResult> {
	if !show_best_challenge() {
		return Ok(MangaPageResult::default());
	}

	let url = format!("{BASE_URL}/bestChallenge/genre?genre=ALL&sort=VIEW&page={page}");
	let html = request(&url)?.html()?;
	let entries = parse_manga_cards(&html);

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

/// Search manga by query or fallback to default
pub fn parse_search_manga_list(
	query: Option<String>,
	page: i32,
	filters: Vec<FilterValue>,
) -> Result<MangaPageResult> {
	let mut search_type = if show_best_challenge() {
		"ALL"
	} else {
		"WEBTOON"
	};

	for filter in &filters {
		if let FilterValue::Select { id, value } = filter
			&& id == "searchType"
		{
			match value.as_str() {
				"WEBTOON" => search_type = "WEBTOON",
				"BEST_CHALLENGE" => search_type = "BEST_CHALLENGE",
				"ALL" => search_type = "ALL",
				_ => {}
			}
		}
	}

	if let Some(ref q) = query {
		let trimmed = q.trim();
		if !trimmed.is_empty() {
			if page > 1 {
				return Ok(MangaPageResult::default());
			}
			let encoded = encode_uri_component(trimmed);
			let url = match search_type {
				"WEBTOON" => {
					format!("{BASE_URL}/search/result?keyword={encoded}&searchType=WEBTOON")
				}
				"BEST_CHALLENGE" => {
					format!("{BASE_URL}/search/result?keyword={encoded}&searchType=BEST_CHALLENGE")
				}
				_ => format!("{BASE_URL}/search/result?keyword={encoded}"),
			};
			let html = request(&url)?.html()?;
			let entries = parse_manga_cards(&html);

			return Ok(MangaPageResult {
				entries,
				has_next_page: false,
			});
		}
	}

	if search_type == "BEST_CHALLENGE" {
		parse_best_challenge_list(page)
	} else {
		parse_weekday_list("mon", page)
	}
}

/// Handles all listings registered in source.json and DynamicListings
pub fn parse_manga_listing(listing: Listing, page: i32) -> Result<MangaPageResult> {
	match listing.id.as_str() {
		"mon" => parse_weekday_list("mon", page),
		"tue" => parse_weekday_list("tue", page),
		"wed" => parse_weekday_list("wed", page),
		"thu" => parse_weekday_list("thu", page),
		"fri" => parse_weekday_list("fri", page),
		"sat" => parse_weekday_list("sat", page),
		"sun" => parse_weekday_list("sun", page),
		"completed" => parse_finish_list(page, "UPDATE"),
		"best" => parse_best_challenge_list(page),
		"popular" => parse_finish_list(page, "ALL_READER"),
		"update" => parse_finish_list(page, "UPDATE"),
		_ => parse_weekday_list("mon", page),
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
		if let Ok(req) = request(&api_url)
			&& let Ok(res) = req.data()
			&& let Ok(info) = serde_json::from_slice::<ApiArticleInfo>(&res)
		{
			if let Some(name) = info.title_name {
				manga.title = name;
			}
			if let Some(cover) = info.poster_thumbnail_url.or(info.thumbnail_url) {
				manga.cover = Some(cover);
			}
			if let Some(desc) = info.synopsis {
				manga.description = Some(desc);
			}
			if let Some(author) = info.display_author {
				let authors: Vec<String> = author
					.split(['/', ','])
					.map(|s| String::from(s.trim()))
					.filter(|s| !s.is_empty())
					.collect();
				if !authors.is_empty() {
					manga.artists = Some(authors.clone());
					manga.authors = Some(authors);
				}
			} else if let Some(artists) = info.community_artists {
				let authors: Vec<String> = artists
					.into_iter()
					.filter_map(|a| a.name)
					.map(|s| String::from(s.trim()))
					.filter(|s| !s.is_empty())
					.collect();
				if !authors.is_empty() {
					manga.artists = Some(authors.clone());
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
		let authors: Vec<String> = a
			.split(['/', ','])
			.map(|s| String::from(s.trim()))
			.filter(|s| !s.is_empty())
			.collect();
		if !authors.is_empty() {
			manga.artists = Some(authors.clone());
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
			if !crate::auth::is_logged_in() {
				return Err(AidokuError::message(
					"로그인이 필요한 작품입니다. 소스 설정에서 네이버 로그인을 완료해주세요.",
				));
			} else {
				return Err(AidokuError::message(
					"네이버 로그인 세션이 만료되었습니다. 소스 설정에서 다시 로그인해주세요.",
				));
			}
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

			let mut chapter_id = node.attr("data-no").unwrap_or_default();
			if chapter_id.is_empty() {
				chapter_id = get_chapter_id(&href);
			}
			if chapter_id.is_empty() || chapters.iter().any(|c| c.key == chapter_id) {
				continue;
			}
			found_new = true;

			let is_locked = node.has_class("lock")
				|| node
					.select_first(".ico_comic .blind")
					.and_then(|e| e.text())
					.map(|t| t.contains("유료"))
					.unwrap_or(false)
				|| (href == "#" && node.select_first("em.cookie_txt").is_some());

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
		if page > 1000 {
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
		return Err(AidokuError::message(
			"로그인이 필요한 작품입니다. 소스 설정에서 네이버 로그인을 완료해주세요.",
		));
	}

	let mut pages: Vec<Page> = Vec::new();
	let mut img_nodes = html.select("img.toon_image");
	if img_nodes.as_ref().is_none_or(|l| l.is_empty()) {
		img_nodes = html.select("div.wt_viewer img, #toon_layer img, div.toon_view_area img");
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
			{
				continue;
			}

			pages.push(Page {
				content: PageContent::url(img_url),
				..Default::default()
			});
		}
	}

	if pages.is_empty() {
		let raw_html = html
			.select_first("body")
			.and_then(|b| b.html())
			.unwrap_or_default();
		if raw_html.contains("유료로 전환된 회차")
			|| raw_html.contains("구매")
			|| raw_html.contains("대여")
		{
			return Err(AidokuError::message(
				"구매 또는 대여가 필요한 유료 회차입니다.",
			));
		}
		return Err(AidokuError::message("회차 이미지를 불러올 수 없습니다."));
	}

	Ok(pages)
}

const TRUSTED_COOKIE_HOSTS: &[&str] = &["comic.naver.com", "m.comic.naver.com"];

fn is_trusted_cookie_host(url: &str) -> bool {
	let after_scheme = if let Some(stripped) = url.strip_prefix("https://") {
		stripped
	} else if let Some(stripped) = url.strip_prefix("http://") {
		stripped
	} else {
		return false;
	};
	let host = after_scheme
		.split(['/', '?', '#', ':'])
		.next()
		.unwrap_or("");
	TRUSTED_COOKIE_HOSTS.contains(&host)
}

/// Handles image request modification (injected Referer + User-Agent + Cookie for trusted hosts)
pub fn get_image_request(url: String) -> Result<Request> {
	let mut req = Request::get(&url)?
		.header("Referer", "https://comic.naver.com/")
		.header("User-Agent", USER_AGENT);
	if is_trusted_cookie_host(&url)
		&& let Some(cookie_str) = crate::auth::get_cookie_header()
	{
		req = req.header("Cookie", &cookie_str);
	}
	Ok(req)
}

/// Handles deep linking for comic.naver.com URLs
pub fn parse_deep_link(url: &str) -> Result<Option<DeepLinkResult>> {
	let manga_key = get_title_id(url);
	if manga_key.is_empty() {
		return Ok(None);
	}
	let chapter_id = get_chapter_id(url);
	if !chapter_id.is_empty() {
		Ok(Some(DeepLinkResult::Chapter {
			manga_key,
			key: chapter_id,
		}))
	} else {
		Ok(Some(DeepLinkResult::Manga { key: manga_key }))
	}
}

/// Parses the home page layout (Today's Webtoons, Popular Completed, and Recent Completed)
pub fn parse_home() -> Result<HomeLayout> {
	let mut components = Vec::new();

	// 1. 오늘의 웹툰 (Today's Webtoons from /webtoon/weekday)
	let today_url = format!("{BASE_URL}/webtoon/weekday");
	if let Ok(req) = request(&today_url)
		&& let Ok(html) = req.html()
	{
		let today_entries: Vec<Link> = parse_manga_cards(&html)
			.into_iter()
			.map(Into::into)
			.collect();
		if !today_entries.is_empty() {
			components.push(HomeComponent {
				title: Some(String::from("오늘의 웹툰")),
				subtitle: None,
				value: HomeComponentValue::Scroller {
					entries: today_entries,
					listing: None,
				},
			});
		}
	}

	// 2. 인기 완결작 (Popular Completed Webtoons)
	if let Ok(pop_result) = parse_finish_list(1, "ALL_READER") {
		let popular_entries: Vec<Link> = pop_result.entries.into_iter().map(Into::into).collect();
		if !popular_entries.is_empty() {
			components.push(HomeComponent {
				title: Some(String::from("인기 완결작")),
				subtitle: None,
				value: HomeComponentValue::Scroller {
					entries: popular_entries,
					listing: Some(Listing {
						id: String::from("popular"),
						name: String::from("인기순"),
						..Default::default()
					}),
				},
			});
		}
	}

	// 3. 최신 완결작 (Recently Updated Completed Webtoons)
	if let Ok(upd_result) = parse_finish_list(1, "UPDATE") {
		let update_entries: Vec<Link> = upd_result.entries.into_iter().map(Into::into).collect();
		if !update_entries.is_empty() {
			components.push(HomeComponent {
				title: Some(String::from("최신 완결작")),
				subtitle: None,
				value: HomeComponentValue::Scroller {
					entries: update_entries,
					listing: Some(Listing {
						id: String::from("update"),
						name: String::from("업데이트순"),
						..Default::default()
					}),
				},
			});
		}
	}

	Ok(HomeLayout { components })
}
