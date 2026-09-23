use aidoku::{
	Chapter, ContentRating, DeepLinkResult, FilterValue, Listing, Manga, MangaPageResult,
	MangaStatus, Page, PageContent, Result, Viewer,
	alloc::{String, Vec, format, vec},
	helpers::uri::encode_uri_component,
	imports::{error::AidokuError, net::Request, std::send_partial_result},
};
use serde::Deserialize;

use crate::helper::*;

#[derive(Deserialize)]
struct ApiResponse {
	result: Option<ApiResultData>,
}

#[derive(Deserialize)]
struct ApiResultData {
	#[serde(rename = "episodeList")]
	episode_list: Option<Vec<ApiEpisode>>,
}

#[derive(Deserialize)]
struct ApiEpisode {
	#[serde(rename = "episodeNo")]
	episode_no: f32,
	#[serde(rename = "episodeTitle")]
	episode_title: Option<String>,
	#[serde(rename = "viewerLink")]
	viewer_link: Option<String>,
	#[serde(rename = "exposureDateMillis")]
	exposure_date_millis: Option<i64>,
	thumbnail: Option<String>,
}

/// Parses search query or falls back to the default genres listing.
pub fn parse_search_manga_list(
	query: Option<String>,
	page: i32,
	filters: Vec<FilterValue>,
) -> Result<MangaPageResult> {
	let base_url = get_base_url(false);

	if let Some(ref q) = query {
		let trimmed = q.trim();
		if !trimmed.is_empty() {
			let encoded = encode_uri_component(trimmed);
			let search_path = if get_canvas_series() {
				"search"
			} else {
				"search/originals"
			};
			let url = format!("{base_url}/{search_path}?keyword={encoded}&page={page}");
			return parse_search_results(&url);
		}
	}

	let mut genre = String::new();
	let mut sort = "UPDATE";

	for filter in filters {
		if let FilterValue::Select { id, value } = filter {
			if id == "genre" {
				genre = value;
			} else if id == "sort" {
				sort = match value.as_str() {
					"MANA" => "MANA",
					"LIKEIT" => "LIKEIT",
					_ => "UPDATE",
				};
			}
		}
	}

	let url = if genre.is_empty() {
		format!("{base_url}/genres?sortOrder={sort}")
	} else {
		format!("{base_url}/genres/{genre}?sortOrder={sort}")
	};

	parse_manga_list(&url, page)
}

/// Parses manga cards from search results with pagination.
fn parse_search_results(url: &str) -> Result<MangaPageResult> {
	let html = request(url, false)?.html()?;
	let mut entries = Vec::new();

	if let Some(items) = html.select("#content > div.webtoon_list_wrap ul > li > a") {
		for node in items {
			let href = node.attr("href").unwrap_or_default();
			let id = get_manga_id(&href);
			if id.is_empty() || entries.iter().any(|m: &Manga| m.key == id) {
				continue;
			}
			let cover = node.select_first("img").and_then(|img| img.attr("src"));
			let title = node
				.select_first(".title")
				.and_then(|t| t.text())
				.unwrap_or_default();
			let full_url = if href.starts_with("http") {
				href
			} else {
				format!("{BASE_URL_DESKTOP}{href}")
			};

			entries.push(Manga {
				key: id,
				title,
				cover,
				url: Some(full_url),
				viewer: Viewer::Webtoon,
				content_rating: ContentRating::Safe,
				..Default::default()
			});
		}
	}

	let has_next_page = !entries.is_empty() && entries.len() >= 12;
	Ok(MangaPageResult {
		entries,
		has_next_page,
	})
}

/// Handles all listings registered in source.json and DynamicListings.
pub fn parse_manga_listing(listing: Listing, page: i32) -> Result<MangaPageResult> {
	let base_url = get_base_url(false);
	match listing.id.as_str() {
		"latest" => parse_manga_list(&format!("{base_url}/genres?sortOrder=UPDATE"), page),
		"popular" => parse_manga_list(&format!("{base_url}/genres?sortOrder=MANA"), page),
		"top" => parse_manga_list(&format!("{base_url}/genres?sortOrder=LIKEIT"), page),
		"canvas_latest" => parse_canvas_list(
			&format!("{base_url}/canvas/list?genreTab=ALL&sortOrder=UPDATE"),
			page,
		),
		"canvas_popular" => parse_canvas_list(
			&format!("{base_url}/canvas/list?genreTab=ALL&sortOrder=READ_COUNT"),
			page,
		),
		"canvas_top" => parse_canvas_list(
			&format!("{base_url}/canvas/list?genreTab=ALL&sortOrder=LIKEIT"),
			page,
		),
		_ => parse_manga_list(&format!("{base_url}/genres"), page),
	}
}

/// Parses manga cards from originals list or search results.
pub fn parse_manga_list(url: &str, page: i32) -> Result<MangaPageResult> {
	if page > 1 {
		return Ok(MangaPageResult::default());
	}

	let html = request(url, false)?.html()?;
	let mut entries = Vec::new();

	if let Some(items) = html.select("#content > div.webtoon_list_wrap ul > li > a") {
		for node in items {
			let href = node.attr("href").unwrap_or_default();
			let id = get_manga_id(&href);
			if id.is_empty() || entries.iter().any(|m: &Manga| m.key == id) {
				continue;
			}
			let cover = node.select_first("img").and_then(|img| img.attr("src"));
			let title = node
				.select_first(".title")
				.and_then(|t| t.text())
				.unwrap_or_default();
			let full_url = if href.starts_with("http") {
				href
			} else {
				format!("{BASE_URL_DESKTOP}{href}")
			};

			entries.push(Manga {
				key: id,
				title,
				cover,
				url: Some(full_url),
				viewer: Viewer::Webtoon,
				content_rating: ContentRating::Safe,
				..Default::default()
			});
		}
	}

	Ok(MangaPageResult {
		entries,
		has_next_page: false,
	})
}

/// Parses manga cards from Canvas list with pagination support.
pub fn parse_canvas_list(url: &str, page: i32) -> Result<MangaPageResult> {
	if !get_canvas_series() {
		return Ok(MangaPageResult::default());
	}

	let paged_url = format!("{url}&page={page}");
	let html = request(&paged_url, false)?.html()?;
	let mut entries = Vec::new();

	if let Some(items) = html.select("#content div.challenge_lst > ul > li > a") {
		for node in items {
			let href = node.attr("href").unwrap_or_default();
			let id = get_manga_id(&href);
			if id.is_empty() || entries.iter().any(|m: &Manga| m.key == id) {
				continue;
			}
			let cover = node.select_first("img").and_then(|img| img.attr("src"));
			let title = node
				.select_first(".subj")
				.and_then(|t| t.text())
				.unwrap_or_default();
			let full_url = if href.starts_with("http") {
				href
			} else {
				format!("{BASE_URL_DESKTOP}{href}")
			};

			entries.push(Manga {
				key: id,
				title,
				cover,
				url: Some(full_url),
				viewer: Viewer::Webtoon,
				content_rating: ContentRating::Safe,
				..Default::default()
			});
		}
	}

	let next_page_param = format!("page={}", page + 1);
	let has_next_page = !entries.is_empty()
		&& (html
			.select_first(format!("a[href*=\"{next_page_param}\"]"))
			.is_some()
			|| html.select_first("a.pg_next").is_some());

	Ok(MangaPageResult {
		entries,
		has_next_page,
	})
}

/// Updates manga details and/or chapter list.
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
		manga.chapters = Some(parse_chapter_list(&manga.key)?);
	}

	Ok(manga)
}

/// Parses webtoon details from the desktop series page.
pub fn parse_manga_details(mut manga: Manga) -> Result<Manga> {
	let url = get_manga_url(&manga.key);
	let html = request(&url, false)?.html()?;

	let cover = html
		.select_first("head meta[property=\"og:image\"]")
		.and_then(|e| e.attr("content"));
	if cover.is_some() {
		manga.cover = cover;
	}

	let title = html
		.select_first(".detail_header .subj, .challenge_header .subj, .info > .subj")
		.and_then(|e| e.text())
		.or_else(|| {
			html.select_first("head meta[property=\"og:title\"]")
				.and_then(|e| e.attr("content"))
		});
	if let Some(t) = title {
		manga.title = t;
	}

	let author_raw = html
		.select_first(".author_area, .author")
		.and_then(|e| e.text())
		.unwrap_or_default()
		.replace("author info", "");
	let parts: Vec<String> = author_raw
		.split([',', '/'])
		.map(|s| s.trim().trim_end_matches('.').trim())
		.filter(|s| !s.is_empty())
		.map(String::from)
		.collect();

	if !parts.is_empty() {
		let first_author = parts[0].clone();
		manga.authors = Some(vec![first_author]);
		if parts.len() > 1 {
			manga.artists = Some(parts.into_iter().skip(1).collect());
		} else if let Some(ref authors) = manga.authors {
			manga.artists = Some(authors.clone());
		}
	}

	let desc = html
		.select_first(".summary")
		.and_then(|e| e.text())
		.or_else(|| {
			html.select_first("head meta[property=\"og:description\"]")
				.and_then(|e| e.attr("content"))
		});
	if desc.is_some() {
		manga.description = desc;
	}

	let status_text = html
		.select_first("#_asideDetail > .day_info")
		.and_then(|e| e.text())
		.unwrap_or_default()
		.to_lowercase();
	let series_note = html
		.select_first("#content > div.cont_box > div.detail_body div.detail_paywall")
		.and_then(|e| e.text())
		.unwrap_or_default()
		.to_lowercase();

	let is_completed = html.select_first(".txt_ico_completed").is_some()
		|| status_text.contains("completed")
		|| status_text.contains("완결")
		|| status_text.contains("terminé")
		|| status_text.contains("completo")
		|| status_text.contains("beendet")
		|| status_text.contains("จบแล้ว")
		|| status_text.contains("已完結")
		|| status_text.contains("tamat");

	let is_hiatus = html.select_first(".txt_ico_hiatus, .ico_hiatus").is_some()
		|| series_note.contains("will return")
		|| series_note.contains("hiatus")
		|| series_note.contains("휴재")
		|| series_note.contains("pause")
		|| series_note.contains("regresará")
		|| series_note.contains("zurückkehren")
		|| series_note.contains("reviendra")
		|| series_note.contains("กลับมา")
		|| series_note.contains("回歸")
		|| series_note.contains("akan kembali");

	manga.status = if is_completed {
		MangaStatus::Completed
	} else if is_hiatus {
		MangaStatus::Hiatus
	} else {
		MangaStatus::Ongoing
	};

	let mut tags = Vec::new();
	if let Some(items) =
		html.select(".detail_header .genre, .challenge_header .genre, .info > .genre")
	{
		for item in items {
			if let Some(text) = item.text() {
				let trimmed = text.trim();
				if !trimmed.is_empty() && !tags.iter().any(|t: &String| t == trimmed) {
					tags.push(String::from(trimmed));
				}
			}
		}
	}
	if !tags.is_empty() {
		manga.tags = Some(tags);
	}

	manga.url = Some(url);
	manga.viewer = Viewer::Webtoon;
	let is_mature = manga.tags.as_ref().is_some_and(|t| {
		t.iter()
			.any(|tag| tag == "Mature" || tag == "Horror" || tag == "Gore")
	}) || html
		.select_first("[data-title-unsuitable-for-children=\"true\"]")
		.is_some();
	manga.content_rating = if is_mature {
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	};

	Ok(manga)
}

/// Cleans episode titles to extract optional volume number and stripped title.
fn clean_episode_title(raw_title: &str) -> (Option<String>, Option<f32>) {
	let mut volume: Option<f32> = None;
	let mut words: Vec<&str> = raw_title.split_whitespace().collect();

	// Remove leading volume text and set volume accordingly: "(S1) Chapter 1 - ..." or "S1 Chapter 1 - ..."
	if !words.is_empty() {
		let chars: Vec<char> = words[0].chars().collect();
		if chars.len() >= 3
			&& (chars[0] == '(' || chars[0] == '[')
			&& (chars[1] == 'S' || chars[1] == 'T' || chars[1] == 's' || chars[1] == 't')
			&& chars[2].is_ascii_digit()
		{
			let digits: String = chars[2..]
				.iter()
				.take_while(|c| c.is_ascii_digit())
				.collect();
			if let Ok(v) = digits.parse::<f32>() {
				volume = Some(v);
				words.remove(0);
			}
		} else if chars.len() >= 2
			&& (chars[0] == 'S' || chars[0] == 's')
			&& chars[1].is_ascii_digit()
		{
			let digits: String = chars[1..]
				.iter()
				.take_while(|c| c.is_ascii_digit())
				.collect();
			if let Ok(v) = digits.parse::<f32>() {
				volume = Some(v);
				words.remove(0);
			}
		} else if chars.len() >= 4
			&& chars[0] == 'E'
			&& (chars[1] == 'p' || chars[1] == 'P')
			&& chars[2] == '.'
			&& chars[3..].iter().all(|c| c.is_ascii_digit())
		{
			// Remove leading episode text: "Ep.1 - ..."
			words.remove(0);
		}
	}

	// Remove leading season text: "[Season 1] Chapter 1 - ..." or "(Season 1) Chapter 1 - ..."
	if words.len() >= 2
		&& (words[0] == "[Season" && words[1].ends_with(']')
			|| words[0] == "(Season" && words[1].ends_with(')'))
	{
		let season_str = words[1].trim_end_matches([']', ')']);
		if let Ok(v) = season_str.parse::<f32>() {
			volume = Some(v);
			words.remove(0);
			words.remove(0);
		}
	}

	// Remove leading chapter/episode text
	if words.len() >= 2 {
		let first = words[0];
		if matches!(
			first,
			"Chapter" | "Episode" | "Ch." | "CH." | "Ep." | "EP" | "EP."
		) {
			let clean_second = words[1].trim_end_matches(':');
			if clean_second.parse::<f32>().is_ok() {
				words.remove(0);
				words.remove(0);
			}
		}
	}

	// Remove leading punctuation symbols
	if !words.is_empty() && (words[0] == "-" || words[0] == ":") {
		words.remove(0);
	}

	let title_str = words.join(" ");
	let title = if title_str.is_empty() {
		None
	} else {
		Some(title_str)
	};

	(title, volume)
}

/// Parses all chapters using the mobile episodes API.
pub fn parse_chapter_list(manga_id: &str) -> Result<Vec<Chapter>> {
	let base_url = get_base_url_no_lang(true);
	let api_url = if let Some(canvas_id) = manga_id.strip_suffix("-canvas") {
		format!("{base_url}/api/v1/canvas/{canvas_id}/episodes?pageSize=100000")
	} else {
		format!("{base_url}/api/v1/webtoon/{manga_id}/episodes?pageSize=100000")
	};

	let res = request(&api_url, true)?.data()?;
	let api_response: ApiResponse = serde_json::from_slice(&res)?;

	let lang = get_lang_code();
	let mut chapters = Vec::new();

	if let Some(result_data) = api_response.result {
		if let Some(episode_list) = result_data.episode_list {
			for episode in episode_list.into_iter().rev() {
				let viewer_link = episode.viewer_link.unwrap_or_default();
				let chapter_id = get_chapter_id(&viewer_link);
				if chapter_id.is_empty() {
					continue;
				}

				let raw_title = episode.episode_title.unwrap_or_default();
				let (title, volume_number) = clean_episode_title(&raw_title);

				let full_chapter_url = if viewer_link.starts_with("http") {
					viewer_link
				} else {
					format!("{BASE_URL_DESKTOP}{viewer_link}")
				};

				let date_uploaded = episode.exposure_date_millis.map(|ms| ms / 1000);

				let thumbnail = episode.thumbnail.map(|t| {
					if t.starts_with("http") {
						t
					} else {
						format!("https://webtoon-phinf.pstatic.net{t}")
					}
				});

				chapters.push(Chapter {
					key: chapter_id,
					title,
					volume_number,
					chapter_number: Some(episode.episode_no),
					date_uploaded,
					url: Some(full_chapter_url),
					thumbnail,
					language: Some(lang.clone()),
					..Default::default()
				});
			}
		}
	}

	Ok(chapters)
}

/// Parses page URLs from viewer HTML.
pub fn parse_page_list(manga_key: &str, chapter_key: &str) -> Result<Vec<Page>> {
	let url = get_chapter_url(chapter_key, manga_key);
	let html = request(&url, false)?.html()?;

	let optional_pages = get_optional_pages();
	let mut pages = Vec::new();

	if let Some(imgs) = html.select("div#_imageList > img") {
		for img in imgs {
			let mut img_url = img.attr("data-url").unwrap_or_default();
			if img_url.is_empty() || img_url.contains("bg_transparency.png") {
				img_url = img.attr("src").unwrap_or_default();
			}
			if img_url.is_empty() || img_url.contains("bg_transparency.png") {
				continue;
			}
			if img_url.ends_with("?type=opti") && !optional_pages {
				continue;
			}
			pages.push(Page {
				content: PageContent::url(img_url),
				..Default::default()
			});
		}
	}

	if pages.is_empty() {
		return Err(AidokuError::message("No pages found"));
	}

	Ok(pages)
}

/// Prepares image request with required headers (Referer & User-Agent).
pub fn get_image_request(url: String) -> Result<Request> {
	Ok(Request::get(url)?
		.header("Referer", BASE_URL_DESKTOP)
		.header("User-Agent", get_user_agent(false)))
}

/// Handles deep linking for webtoons.com URLs.
pub fn parse_deep_link(url: &str) -> Result<Option<DeepLinkResult>> {
	let manga_key = get_manga_id(url);
	if manga_key.is_empty() {
		return Ok(None);
	}
	let chapter_key = get_chapter_id(url);
	if !chapter_key.is_empty() {
		Ok(Some(DeepLinkResult::Chapter {
			manga_key,
			key: chapter_key,
		}))
	} else {
		Ok(Some(DeepLinkResult::Manga { key: manga_key }))
	}
}
