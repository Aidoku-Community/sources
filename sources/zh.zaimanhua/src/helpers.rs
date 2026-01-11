use crate::models;
use crate::net;
use crate::settings;
use aidoku::{
	FilterValue, Manga, MangaPageResult, Result,
	alloc::{String, Vec, format, string::ToString},
	error,
	imports::net::Request,
};
use hashbrown::HashSet;


// === Search Logic ===

pub fn search_by_keyword(keyword: &str, page: i32) -> Result<MangaPageResult> {
	if keyword.trim().is_empty() {
		return Ok(MangaPageResult::default());
	}

	// Progressive Hidden Search: Map search page to hidden content batch
	// Page 1 -> Hidden 1-5 (Start: 1)
	// Page 2 -> Hidden 6-10 (Start: 6)
	let hidden_start_page = (page - 1) * 5 + 1;
	let mut hidden_has_next = false;
	let keyword_lower = keyword.to_lowercase();

	// standard source query
	let search_response: models::ApiResponse<models::SearchData> =
		net::Url::Search { keyword, page, size: 20 }.request()?.json_owned()?;
	let search_data = search_response.data.ok_or_else(|| error!("Missing data"))?;

	let mut search_results: Vec<Manga> = search_data.list.into_iter().map(Into::into).collect();
	let search_total = search_data.total.unwrap_or(0) as i32;
	let has_next_page = (page * 20) < search_total;

	// hidden content inclusion (scanner mode with auto-skip)
	if settings::show_hidden_content() {
		// Use HiddenContentScanner to lazily fetch up to 3 batches (1500 items max)
		// skipping empty batches automatically.
		let scanner = net::HiddenContentScanner::new(hidden_start_page, 3);
		
		for hidden_items in scanner {
			hidden_has_next = hidden_items.len() >= 500;

			let existing_ids: HashSet<String> =
				search_results.iter().map(|m| m.key.clone()).collect();
			
			let before_count = search_results.len();
			search_results.extend(
				hidden_items
					.into_iter()
					.filter(|item| !existing_ids.contains(&item.id.to_string()))
					.filter(|item| {
						let name_lower = item.name.to_lowercase();
						let auth_lower = item.authors.as_deref().unwrap_or("").to_lowercase();
						name_lower.contains(&keyword_lower) || auth_lower.contains(&keyword_lower)
					})
					.map(Into::into)
			);
			let after_count = search_results.len();

			// If we found any results, we stop scanning.
			// Otherwise the iterator continues to the next batch.
			if after_count > before_count {
				break;
			}
		}
	}

	Ok(MangaPageResult {
		entries: search_results,
		has_next_page: has_next_page || hidden_has_next,
	})
}

// === Filter & Browse Logic ===

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


	if let Some(mode @ ("1" | "2" | "3" | "4")) = rank_mode {
		let by_time = mode.parse::<i32>().unwrap_or(1) - 1;
		let response: models::ApiResponse<Vec<models::RankItem>> =
			net::Url::Rank { by_time, page }.request()?.json_owned()?;
		let data = response.data.unwrap_or_default();
		return Ok(models::manga_list_from_ranks(data));
	}


	let params = format!(
		"sortType={}&cate={}&status={}&zone={}&theme={}",
		sort_type.unwrap_or("1"),
		cate.unwrap_or("0"),
		status.unwrap_or("0"),
		zone.unwrap_or("0"),
		theme.unwrap_or("0")
	);

	let response: models::ApiResponse<models::FilterData> =
		net::Url::Filter { params: &params, page, size: 20 }.request()?.json_owned()?;
	let data = response.data.map(|d| d.comic_list).unwrap_or_default();
	Ok(models::manga_list_from_filter(data))
}

pub fn search_by_author(author: &str, page: i32) -> Result<MangaPageResult> {

	let author_matches = |manga_authors: &str| -> bool {
		// Split by separators to prevent partial matches (e.g. "D" matching "David")
		let separators = ['/', ',', '，', '、', '&', ';'];
		let parts = manga_authors.split(|c| separators.contains(&c));
		
		for part in parts {
			let trimmed = part.trim();
			// Case-insensitive exact match
			if trimmed.eq_ignore_ascii_case(author) {
				return true;
			}
		}

		// Allow loose matching only for specific queries (Multibyte or >3 chars)
		// to avoid noisy matches on short ASCII tokens.
		let is_safe_query = author.chars().any(|c| c.len_utf8() > 1) || author.len() > 3;

		if is_safe_query && manga_authors.to_lowercase().contains(&author.to_lowercase()) {
			return true;
		}

		false
	};

	let mut all_tag_ids: Vec<i64> = Vec::new();
	let mut keyword_manga: Vec<models::SearchItem> = Vec::new();
	let mut seen_authors: Vec<String> = Vec::new();

	if let Ok(response) = (net::Url::Search { keyword: author, page: 1, size: 50 }).request()?.json_owned::<models::ApiResponse<models::SearchData>>()
		&& let Some(data) = response.data
	{
		for item in data.list {
			let manga_authors = item.authors.as_deref().unwrap_or("");

			if author_matches(manga_authors) {
				let author_key = manga_authors.to_string();
				if !seen_authors.contains(&author_key) {
					seen_authors.push(author_key);
					let _ = collect_author_tags(item.id, author, &mut all_tag_ids);
				}
				keyword_manga.push(item);
			}
		}
	}

	// fallback: try simplified name if exact match fails
	if all_tag_ids.is_empty() && keyword_manga.is_empty() {
		let core_name = author;
		let short_core = if core_name.chars().count() >= 4 {
			core_name.chars().take(2).collect::<String>()
		} else {
			core_name.to_string()
		};

		for core in [core_name, short_core.as_str()] {
			if core.is_empty() || core == author || !all_tag_ids.is_empty() {
				continue;
			}

			if let Ok(response) =
				(net::Url::Search { keyword: core, page: 1, size: 30 }).request()?.json_owned::<models::ApiResponse<models::SearchData>>()
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
							let _ = collect_author_tags(item.id, author, &mut all_tag_ids);
						}
						keyword_manga.push(item);
					}
				}
			}
		}
	}

	// Fetch works by author tag IDs
	let (tag_manga, tag_total): (Vec<models::FilterItem>, i32) = if !all_tag_ids.is_empty() {
		let tag_requests: Vec<_> = all_tag_ids
			.iter()
			.filter_map(|tid| {
				net::Url::Theme { theme_id: *tid, page, size: 100 }.request().ok()
			})
			.collect();

		let items: Vec<models::FilterItem> = Request::send_all(tag_requests)
			.into_iter()
			.flatten()
			.filter_map(|resp| {
				resp.get_json_owned::<models::ApiResponse<models::FilterData>>()
					.ok()
					.and_then(|r| r.data)
			})
			.flat_map(|data| data.comic_list)
			.collect();

		let total = items.len() as i32;
		(items, total)
	} else {
		(Vec::new(), 0)
	};

	// Merge and deduplicate
	let mut seen_ids: HashSet<i64> = HashSet::new();
	let mut final_manga: Vec<Manga> = Vec::new();

	// hidden content inclusion (scanner mode with auto-skip)
	if settings::show_hidden_content() {
		let hidden_start_page = (page - 1) * 5 + 1;
		let scanner = net::HiddenContentScanner::new(hidden_start_page, 3);

		for items in scanner {
			if items.len() < 500 {
				// If batch isn't full, we probably reached end of library
			}

			let mut batch_found_any = false;
			for item in items {
				let authors = item.authors.as_deref().unwrap_or("");
				if author_matches(authors) && !seen_ids.contains(&item.id) {
					seen_ids.insert(item.id);
					final_manga.push(item.into());
					batch_found_any = true;
				}
			}

			if batch_found_any {
				break;
			}
		}
	}

	// Add tag-based and keyword search results
	let tag_iter = tag_manga.into_iter().map(|i| -> Manga { i.into() });
	let keyword_iter = keyword_manga.into_iter().map(|i| -> Manga { i.into() });
	for manga in tag_iter.chain(keyword_iter) {
		if let Ok(id) = manga.key.parse::<i64>()
			&& id > 0 && !seen_ids.contains(&id)
		{
			seen_ids.insert(id);
			final_manga.push(manga);
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

// === Helper Functions ===

fn collect_author_tags(manga_id: i64, target_author: &str, tag_ids: &mut Vec<i64>) -> Result<()> {
	let manga_id_str = format!("{}", manga_id);

	if let Ok(response) =
		(net::Url::Manga { id: &manga_id_str }).request()?.json_owned::<models::ApiResponse<models::DetailData>>()
		&& let Some(detail_data) = response.data
		&& let Some(detail) = detail_data.data
		&& let Some(authors) = detail.authors
	{
		// Prioritize exact match
		for author in &authors {
			let Some(name) = &author.tag_name else { continue };
			let Some(tid) = author.tag_id else { continue };

			if tid > 0 && !tag_ids.contains(&tid) && name == target_author {
				tag_ids.push(tid);
				return Ok(()); // Exact match found, done
			}
		}

		// Fallback to fuzzy match
		for author in authors {
			let Some(name) = &author.tag_name else { continue };
			let Some(tid) = author.tag_id else { continue };
			
			if tid > 0
				&& !tag_ids.contains(&tid)
				&& (name.contains(target_author) || target_author.contains(name.as_str()))
			{
				tag_ids.push(tid);
			}
		}
	}
	Ok(())
}
