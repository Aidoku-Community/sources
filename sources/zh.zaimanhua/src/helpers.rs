use crate::models;
use crate::net;
use crate::settings;
use aidoku::{
	FilterValue, Manga, MangaPageResult, Result,
	alloc::{String, Vec, format, string::ToString},
	error,
	helpers::uri::encode_uri_component,
	imports::{net::Request, std::send_partial_result},
};
use hashbrown::HashSet;

pub const V4_API_URL: &str = "https://v4api.zaimanhua.com/app/v1";

/// Create a GET request, attaching auth token if Enhanced Mode is active.
pub fn get_api_request(url: &str) -> Result<Request> {
	if let Some(token) = settings::get_token() {
		// Only use auth if enhanced mode is enabled
		if settings::get_enhanced_mode() {
			net::auth_request(url, &token)
		} else {
			net::get_request(url)
		}
	} else {
		net::get_request(url)
	}
}

/// Fetch hidden content with parallel pagination (5 pages × 100 items).
fn fetch_hidden_parallel() -> Vec<models::FilterItem> {
	let page_size = 100;
	let num_pages = 5;

	// Build parallel requests
	let requests: Vec<Request> = (1..=num_pages)
		.filter_map(|p| {
			let url = format!(
				"{}/comic/filter/list?sortType=1&page={}&size={}",
				V4_API_URL, p, page_size
			);
			get_api_request(&url).ok()
		})
		.collect();

	let responses = Request::send_all(requests);

	let mut all_items: Vec<models::FilterItem> = Vec::new();
	let mut cache_entries: Vec<String> = Vec::new();

	// Process all responses
	for resp in responses.into_iter().flatten() {
		if let Ok(filter_response) =
			resp.get_json_owned::<models::ApiResponse<models::FilterData>>()
			&& let Some(filter_data) = filter_response.data
		{
			for item in filter_data.comic_list {
				let id_str = item.id.to_string();
				let name = &item.name;
				let authors = item.authors.as_deref().unwrap_or("");
				cache_entries.push(format!("{}|{}|{}", id_str, name, authors));
				all_items.push(item);
			}
		}
	}

	// Update cache
	if !cache_entries.is_empty() {
		settings::set_hidden_cache(&cache_entries.join("\n"));
	}

	all_items
}

pub fn search_by_keyword(keyword: &str, page: i32) -> Result<MangaPageResult> {
	if keyword.trim().is_empty() {
		return Ok(MangaPageResult::default());
	}

	let should_search_hidden = settings::show_hidden_content() && page == 1;
	let keyword_lower = keyword.to_lowercase();

	// Step 1: Cache-first - show cached matches IMMEDIATELY (before any API call)
	if should_search_hidden
		&& let Some(cached) = settings::get_hidden_cache()
		&& settings::is_hidden_cache_valid()
	{
			let mut cached_matches: Vec<Manga> = Vec::new();
			for line in cached.lines() {
				let parts: Vec<&str> = line.splitn(3, '|').collect();
				if parts.len() < 3 {
					continue;
				}
				let id = parts[0];
				let name = parts[1];
				let authors = parts[2];

				let title_match = name.to_lowercase().contains(&keyword_lower);
				let author_match = authors.to_lowercase().contains(&keyword_lower);

				if title_match || author_match {
					cached_matches.push(Manga {
						key: id.to_string(),
						title: name.to_string(),
						authors: if authors.is_empty() {
							None
						} else {
							Some(aidoku::alloc::vec![authors.to_string()])
						},
						..Default::default()
					});
				}
			}

			// Send cached matches immediately
			if !cached_matches.is_empty() {
				send_partial_result(&MangaPageResult {
					entries: cached_matches,
					has_next_page: true, // Will be corrected in final result
				});
			}
	}

	// Step 2: Fetch Search API
	let encoded = encode_uri_component(keyword);
	let search_url = format!(
		"{}/search/index?keyword={}&source=0&page={}&size=20",
		V4_API_URL, encoded, page
	);

	let search_response: models::ApiResponse<models::SearchData> =
		get_api_request(&search_url)?.json_owned()?;
	let search_data = search_response.data.ok_or_else(|| error!("Missing data"))?;

	let mut search_results: Vec<Manga> = search_data.list.into_iter().map(Into::into).collect();
	let search_total = search_data.total.unwrap_or(0) as i32;
	let has_next_page = (page * 20) < search_total;

	// Step 3: If hidden content enabled, merge with cache or fetch fresh
	if should_search_hidden {
		if let Some(cached) = settings::get_hidden_cache()
			&& settings::is_hidden_cache_valid()
		{
			// Use cache for merge
			merge_cached_hidden(&keyword_lower, &cached, &mut search_results);
		} else {
			// Cache miss - fetch with parallel pagination
			let hidden_items = fetch_hidden_parallel();
			let existing_ids: HashSet<String> =
				search_results.iter().map(|m| m.key.clone()).collect();

			for item in hidden_items {
				let id_str = item.id.to_string();
				if existing_ids.contains(&id_str) {
					continue;
				}

				let name = &item.name;
				let authors = item.authors.as_deref().unwrap_or("");
				let title_match = name.to_lowercase().contains(&keyword_lower);
				let author_match = authors.to_lowercase().contains(&keyword_lower);

				if title_match || author_match {
					search_results.push(item.into());
				}
			}
		}
	}

	Ok(MangaPageResult {
		entries: search_results,
		has_next_page,
	})
}

/// Merge hidden content from cache into search results
fn merge_cached_hidden(keyword: &str, cached: &str, results: &mut Vec<Manga>) {
	// Use HashSet for O(1) lookup (own the strings to avoid borrow issues)
	let existing_ids: HashSet<String> = results.iter().map(|m| m.key.clone()).collect();

	for line in cached.lines() {
		let parts: Vec<&str> = line.splitn(3, '|').collect();
		if parts.len() < 3 {
			continue;
		}

		let id = parts[0];
		let name = parts[1];
		let authors = parts[2];

		// Skip if already in results (O(1) HashSet check)
		if existing_ids.contains(&id.to_string()) {
			continue;
		}

		// Fuzzy match
		let title_match = name.to_lowercase().contains(keyword);
		let author_match = authors.to_lowercase().contains(keyword);

		if title_match || author_match {
			results.push(Manga {
				key: id.to_string(),
				title: name.to_string(),
				authors: if authors.is_empty() {
					None
				} else {
					Some(aidoku::alloc::vec![authors.to_string()])
				},
				..Default::default()
			});
		}
	}
}

/// Browse manga with filters (including optional rank mode)
pub fn browse_with_filters(filters: &[FilterValue], page: i32) -> Result<MangaPageResult> {
	let mut sort_type: Option<&str> = None;
	let mut zone: Option<&str> = None;
	let mut status: Option<&str> = None;
	let mut cate: Option<&str> = None;
	let mut theme: Option<&str> = None;
	let mut rank_mode: Option<&str> = None;

	for filter in filters {
		if let FilterValue::Select { id, value } = filter {
			match id.as_str() {
				"排序" => sort_type = Some(value.as_str()),
				"地区" => zone = Some(value.as_str()),
				"状态" => status = Some(value.as_str()),
				"受众" => cate = Some(value.as_str()),
				"题材" => theme = Some(value.as_str()),
				"榜单" => rank_mode = Some(value.as_str()),
				_ => {}
			}
		}
	}

	let url = match rank_mode {
		Some("1") => format!(
			"{}/comic/rank/list?rank_type=0&by_time=0&page={}&size=20",
			V4_API_URL, page
		),
		Some("2") => format!(
			"{}/comic/rank/list?rank_type=0&by_time=1&page={}&size=20",
			V4_API_URL, page
		),
		Some("3") => format!(
			"{}/comic/rank/list?rank_type=0&by_time=2&page={}&size=20",
			V4_API_URL, page
		),
		Some("4") => format!(
			"{}/comic/rank/list?rank_type=0&by_time=3&page={}&size=20",
			V4_API_URL, page
		),
		_ => format!(
			"{}/comic/filter/list?sortType={}&cate={}&status={}&zone={}&theme={}&page={}&size=20",
			V4_API_URL,
			sort_type.unwrap_or("1"),
			cate.unwrap_or("0"),
			status.unwrap_or("0"),
			zone.unwrap_or("0"),
			theme.unwrap_or("0"),
			page
		),
	};

	if rank_mode.is_some() {
		let response: models::ApiResponse<Vec<models::RankItem>> =
			get_api_request(&url)?.json_owned()?;
		let data = response.data.ok_or_else(|| error!("Missing data"))?;
		Ok(models::manga_list_from_ranks(data))
	} else {
		let response: models::ApiResponse<models::FilterData> =
			get_api_request(&url)?.json_owned()?;
		let data = response.data.ok_or_else(|| error!("Missing data"))?;
		Ok(models::manga_list_from_filter(data.comic_list))
	}
}

pub fn search_by_author(author: &str, page: i32) -> Result<MangaPageResult> {
	let encoded = encode_uri_component(author);

	let author_matches = |manga_authors: &str| -> bool {
		if manga_authors.contains(author) {
			return true;
		}
		for part in manga_authors.split('/') {
			let trimmed = part.trim();
			if !trimmed.is_empty() && (trimmed.contains(author) || author.contains(trimmed)) {
				return true;
			}
		}
		false
	};

	let mut all_tag_ids: Vec<i64> = Vec::new();
	let mut keyword_manga: Vec<models::SearchItem> = Vec::new();
	let mut seen_authors: Vec<String> = Vec::new();

	// Step 1: Search for manga by author name
	let search_url = format!(
		"{}/search/index?keyword={}&source=0&page=1&size=50",
		V4_API_URL, encoded
	);

	if let Ok(response) =
		get_api_request(&search_url)?.json_owned::<models::ApiResponse<models::SearchData>>()
		&& let Some(data) = response.data
	{
		for item in data.list {
			let manga_authors = item.authors.as_deref().unwrap_or("");

			if author_matches(manga_authors) {
				let author_key = manga_authors.to_string();
				if !seen_authors.contains(&author_key) {
					seen_authors.push(author_key);
					let _ = collect_author_tags(item.id, &mut all_tag_ids);
				}
				keyword_manga.push(item);
			}
		}
	}

	// Step 2: Fallback core name search if no results
	if all_tag_ids.is_empty() && keyword_manga.is_empty() {
		let core_name = author
			.trim_start_matches('◎')
			.trim_start_matches('@')
			.trim_start_matches('◯');
		let short_core = if core_name.chars().count() >= 4 {
			core_name.chars().take(2).collect::<String>()
		} else {
			core_name.to_string()
		};

		for core in [core_name, short_core.as_str()] {
			if core.is_empty() || core == author || !all_tag_ids.is_empty() {
				continue;
			}

			let core_encoded = encode_uri_component(core);
			let core_url = format!(
				"{}/search/index?keyword={}&source=0&page=1&size=30",
				V4_API_URL, core_encoded
			);

			if let Ok(response) =
				get_api_request(&core_url)?.json_owned::<models::ApiResponse<models::SearchData>>()
				&& let Some(data) = response.data
			{
				for item in data.list {
					if !all_tag_ids.is_empty() {
						break;
					}

					let manga_authors = item.authors.as_deref().unwrap_or("");
					if manga_authors.contains(core) {
						let author_key = manga_authors.to_string();
						if !seen_authors.contains(&author_key) {
							seen_authors.push(author_key);
							let _ = collect_author_tags(item.id, &mut all_tag_ids);
						}
						keyword_manga.push(item);
					}
				}
			}
		}
	}

	// Step 3: Use tag_ids to get complete works (parallel requests)
	let mut tag_manga: Vec<models::FilterItem> = Vec::new();
	let mut tag_total = 0i32;

	if !all_tag_ids.is_empty() {
		let tag_requests: Vec<_> = all_tag_ids
			.iter()
			.filter_map(|tid| {
				let furl = format!(
					"{}/comic/filter/list?theme={}&page={}&size=100",
					V4_API_URL, tid, page
				);
				net::get_request(&furl).ok()
			})
			.collect();

		let tag_responses = Request::send_all(tag_requests);

		for resp_result in tag_responses {
			if let Ok(resp) = resp_result
				&& let Ok(response) =
					resp.get_json_owned::<models::ApiResponse<models::FilterData>>()
				&& let Some(data) = response.data
			{
				tag_total = tag_total.max(data.comic_list.len() as i32);
				tag_manga.extend(data.comic_list);
			}
		}
	}

	// Step 4: Merge and deduplicate (O(1) with HashSet)
	let mut seen_ids: HashSet<i64> = HashSet::new();
	let mut final_manga: Vec<Manga> = Vec::new();

	// Step 4a: If hidden content enabled, search from cache or API
	if settings::show_hidden_content() && page == 1 {
		// Try cache first
		if let Some(cached) = settings::get_hidden_cache()
			&& settings::is_hidden_cache_valid()
		{
			// Parse cached data and filter by author
			for line in cached.lines() {
				let parts: Vec<&str> = line.splitn(3, '|').collect();
				if parts.len() < 3 {
					continue;
				}
				let id_str = parts[0];
				let _name = parts[1];
				let authors = parts[2];

				if let Ok(id) = id_str.parse::<i64>()
					&& author_matches(authors)
					&& !seen_ids.contains(&id)
				{
					seen_ids.insert(id);
					final_manga.push(Manga {
						key: id_str.to_string(),
							title: parts[1].to_string(),
							authors: if authors.is_empty() {
								None
							} else {
								Some(aidoku::alloc::vec![authors.to_string()])
							},
							..Default::default()

					});
				}
			}
		} else {
			// Cache miss - fetch with parallel pagination
			let hidden_items = fetch_hidden_parallel();
			for item in hidden_items {
				let manga_authors = item.authors.as_deref().unwrap_or("");
				if author_matches(manga_authors) && !seen_ids.contains(&item.id) {
					seen_ids.insert(item.id);
					final_manga.push(item.into());
				}
			}
		}
	}

	// Step 4b: Add results from tag-based search
	for item in tag_manga {
		let id = item.id;
		if id > 0 && !seen_ids.contains(&id) {
			seen_ids.insert(id);
			final_manga.push(item.into());
		}
	}

	// Step 4c: Add results from keyword search

	for item in keyword_manga {
		let id = item.id;
		if id > 0 && !seen_ids.contains(&id) {
			seen_ids.insert(id);
			final_manga.push(item.into());
		}
	}

	if !final_manga.is_empty() {
		let has_next = if tag_total > 0 {
			(page * 100) < tag_total
		} else {
			final_manga.len() >= 100
		};
		return Ok(MangaPageResult {
			entries: final_manga,
			has_next_page: has_next,
		});
	}

	Ok(MangaPageResult::default())
}

fn collect_author_tags(manga_id: i64, tag_ids: &mut Vec<i64>) -> Result<()> {
	let detail_url = format!("{}/comic/detail/{}?channel=android", V4_API_URL, manga_id);

	if let Ok(response) =
		get_api_request(&detail_url)?.json_owned::<models::ApiResponse<models::DetailData>>()
		&& let Some(detail_data) = response.data
		&& let Some(detail) = detail_data.data
		&& let Some(authors) = detail.authors
	{
		for author in authors {
			if let Some(tid) = author.tag_id
				&& tid > 0 && !tag_ids.contains(&tid)
			{
				tag_ids.push(tid);
			}
		}
	}
	Ok(())
}
