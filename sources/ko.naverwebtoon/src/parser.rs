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
	genres: Option<Vec<ApiGenreInfo>>,
	#[serde(rename = "challengeTagList")]
	challenge_tag_list: Option<Vec<String>>,
	age: Option<ApiAgeInfo>,
	adult: Option<bool>,
}

#[derive(Deserialize)]
struct ApiGenreInfo {
	description: Option<String>,
}

#[derive(Deserialize)]
struct ApiAgeInfo {
	#[serde(rename = "type")]
	age_type: Option<String>,
}

#[derive(Deserialize)]
struct ApiCommunityArtist {
	name: Option<String>,
	#[serde(rename = "artistTypeList")]
	artist_type_list: Option<Vec<String>>,
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
	#[serde(rename = "webtoonLevelCode")]
	webtoon_level_code: Option<String>,
}

#[derive(Deserialize)]
struct ApiArticleListResponse {
	#[serde(rename = "articleList")]
	article_list: Option<Vec<ApiArticleItem>>,
	#[serde(rename = "pageInfo")]
	page_info: Option<ApiArticlePageInfo>,
}

#[derive(Deserialize)]
struct ApiArticlePageInfo {
	#[serde(rename = "totalPages")]
	total_pages: Option<i32>,
}

#[derive(Deserialize)]
struct ApiArticleItem {
	no: i64,
	subtitle: Option<String>,
	#[serde(rename = "thumbnailUrl")]
	thumbnail_url: Option<String>,
	#[serde(rename = "serviceDateDescription")]
	service_date_description: Option<String>,
	charge: Option<bool>,
	#[serde(rename = "thumbnailLock")]
	thumbnail_lock: Option<bool>,
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
	let mut search_type = "WEBTOON";
	let mut weekday = "";
	let mut sort_index: i32 = 0;
	let mut search_query = query.as_deref();

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
			FilterValue::Text { id, value }
				if id == "author" && !value.is_empty() && search_query.is_none() =>
			{
				search_query = Some(value.as_str());
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

	if let Some(q) = search_query {
		let trimmed = q.trim();
		if !trimmed.is_empty() {
			let encoded = encode_uri_component(trimmed);

			let api_url = if search_type == "BEST_CHALLENGE" {
				format!("{API_URL}/search/bestChallenge?keyword={encoded}&page={page}")
			} else {
				format!("{API_URL}/search/webtoon?keyword={encoded}&page={page}")
			};

			let resp: ApiSearchResponse = request(&api_url)?.json_owned()?;
			let has_next_page = resp.page_info.and_then(|p| p.next_page).unwrap_or(0) != 0;
			let entries = resp
				.search_list
				.unwrap_or_default()
				.into_iter()
				.filter_map(|item| {
					let title = item.title_name.filter(|t| !t.trim().is_empty())?;
					let is_best = item.webtoon_level_code.as_deref() == Some("BEST_CHALLENGE")
						|| search_type == "BEST_CHALLENGE";
					let key = if is_best {
						format!("{}-best", item.title_id)
					} else {
						item.title_id.to_string()
					};
					let url = Some(get_manga_url(&key));

					Some(Manga {
						key,
						title,
						cover: item.thumbnail_url,
						url,
						..Default::default()
					})
				})
				.collect();

			return Ok(MangaPageResult {
				entries,
				has_next_page,
			});
		}
	}

	// Filter-based browse (no search query)
	if search_type == "BEST_CHALLENGE" {
		parse_best_challenge_list(page, best_challenge_sort)
	} else if weekday == "finish" {
		parse_finish_list(page, webtoon_sort)
	} else {
		parse_weekday_list(weekday, webtoon_sort)
	}
}

/// Parses webtoon details (title, cover, author, description, status, genre, 19+ check)
pub fn parse_manga_details(mut manga: Manga) -> Result<Manga> {
	let clean_id = manga.key.strip_suffix("-best").unwrap_or(&manga.key);
	let api_url = format!("{API_URL}/article/list/info?titleId={clean_id}");
	let info: ApiArticleInfo = request(&api_url)?.json_owned()?;

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
		let mut authors = Vec::new();
		let mut painters = Vec::new();

		for artist in artists {
			let Some(name) = artist
				.name
				.map(|s| String::from(s.trim()))
				.filter(|s| !s.is_empty())
			else {
				continue;
			};
			let types = artist.artist_type_list.unwrap_or_default();
			let is_writer = types.iter().any(|t| t == "ARTIST_WRITER");
			let is_painter = types.iter().any(|t| t == "ARTIST_PAINTER");

			if is_writer && is_painter {
				authors.push(name.clone());
				painters.push(name);
			} else if is_writer || !is_painter {
				authors.push(name);
			} else if is_painter {
				painters.push(name);
			}
		}

		if !authors.is_empty() {
			manga.authors = Some(authors);
		}
		if !painters.is_empty() {
			manga.artists = Some(painters);
		}
	}

	let mut tag_names: Vec<String> = Vec::new();

	if let Some(tags) = info.curation_tag_list {
		for tag in tags {
			if let Some(name) = tag.tag_name {
				let trimmed = name.trim();
				if !trimmed.is_empty() && !tag_names.iter().any(|t| t == trimmed) {
					tag_names.push(String::from(trimmed));
				}
			}
		}
	}

	if let Some(genres) = info.genres {
		for genre in genres {
			if let Some(desc) = genre.description {
				let trimmed = desc.trim();
				if !trimmed.is_empty() && !tag_names.iter().any(|t| t == trimmed) {
					tag_names.push(String::from(trimmed));
				}
			}
		}
	}

	if let Some(tags) = info.challenge_tag_list {
		for tag in tags {
			let trimmed = tag.trim();
			if !trimmed.is_empty() && !tag_names.iter().any(|t| t == trimmed) {
				tag_names.push(String::from(trimmed));
			}
		}
	}

	if !tag_names.is_empty() {
		manga.tags = Some(tag_names);
	}
	manga.status = if info.finished.unwrap_or(false) {
		MangaStatus::Completed
	} else if info.rest.unwrap_or(false) {
		MangaStatus::Hiatus
	} else {
		MangaStatus::Ongoing
	};

	let age_type = info
		.age
		.as_ref()
		.and_then(|a| a.age_type.as_deref())
		.unwrap_or("");
	let is_adult = age_type == "RATE_18"
		|| age_type == "RATE_19"
		|| info.adult.unwrap_or(false)
		|| manga
			.tags
			.as_ref()
			.is_some_and(|tags| tags.iter().any(|t| t.contains("19") || t.contains("성인")));

	manga.content_rating = if is_adult {
		ContentRating::NSFW
	} else if age_type == "RATE_15" {
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	};

	if is_adult {
		if let Some(ref mut tags) = manga.tags {
			if !tags.iter().any(|t| t == "성인") {
				tags.push(String::from("성인"));
			}
		} else {
			manga.tags = Some(vec![String::from("성인")]);
		}
	}

	manga.url = Some(get_manga_url(&manga.key));
	manga.viewer = Viewer::Webtoon;
	Ok(manga)
}

/// Helper to parse chapter list via official JSON API
fn parse_chapter_list_api(manga_id: &str) -> Result<Vec<Chapter>> {
	let clean_id = manga_id.strip_suffix("-best").unwrap_or(manga_id);
	let mut chapters: Vec<Chapter> = Vec::new();
	let mut page = 1;

	loop {
		let api_url = format!("{API_URL}/article/list?titleId={clean_id}&page={page}&sort=DESC");
		let resp = request(&api_url)?.send()?;

		if resp.status_code() == 401 {
			bail!(
				"로그인이 필요한 성인(19+) 작품입니다. 소스 설정에서 네이버 로그인을 진행해주세요."
			);
		}

		let list_resp: ApiArticleListResponse = resp.get_json_owned()?;
		let total_pages = list_resp
			.page_info
			.as_ref()
			.and_then(|p| p.total_pages)
			.unwrap_or(1);

		let items = list_resp.article_list.unwrap_or_default();
		if items.is_empty() {
			break;
		}

		for item in items {
			let chapter_id = item.no.to_string();
			if chapters.iter().any(|c| c.key == chapter_id) {
				continue;
			}

			let raw_title = item.subtitle.unwrap_or_else(|| format!("{chapter_id}화"));
			let fallback_no = item.no as f32;
			let chapter_num = extract_chapter_number(&raw_title, fallback_no);
			let volume_number = extract_volume_number(&raw_title);
			let date_uploaded = item
				.service_date_description
				.as_deref()
				.and_then(parse_korean_date);
			let locked = item.charge.unwrap_or(false) || item.thumbnail_lock.unwrap_or(false);

			let chapter_url = get_chapter_url(&chapter_id, manga_id);

			chapters.push(Chapter {
				key: chapter_id,
				title: Some(raw_title),
				volume_number,
				chapter_number: (chapter_num >= 0.0).then_some(chapter_num),
				date_uploaded,
				url: Some(chapter_url),
				thumbnail: item.thumbnail_url,
				locked,
				..Default::default()
			});
		}

		if page >= total_pages || page >= 500 {
			break;
		}
		page += 1;
	}

	Ok(chapters)
}

/// Parses all chapters for a given manga
pub fn parse_chapter_list(manga_id: &str) -> Result<Vec<Chapter>> {
	parse_chapter_list_api(manga_id)
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
		bail!("성인(19+) 웹툰입니다. 소스 설정에서 네이버 로그인을 진행해주세요.");
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

/// Handles image request modification (injected Referer + User-Agent, NO session cookies)
pub fn get_image_request(url: String) -> Result<Request> {
	Ok(Request::get(url)?
		.header("Referer", "https://comic.naver.com/")
		.header("User-Agent", USER_AGENT))
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
