use crate::models;
use crate::net;
use crate::settings;
use aidoku::{
	Manga, MangaPageResult, Result,
	alloc::{String, Vec, string::ToString, vec},
	imports::net::Request,
};
use hashbrown::HashSet;

// === Public Types ===

/// Encapsulates result merging with ID-based deduplication.
/// Uses i64 IDs for deduplication to avoid string allocation overhead.
pub struct MangaMerger {
	seen_ids: HashSet<i64>,
	results: Vec<Manga>,
}

impl MangaMerger {
	pub fn new() -> Self {
		Self {
			seen_ids: HashSet::new(),
			results: Vec::new(),
		}
	}

	/// Try to add a manga by its numeric ID. Returns true if added (not duplicate).
	pub fn try_add(&mut self, id: i64, manga: Manga) -> bool {
		if id > 0 && !self.seen_ids.contains(&id) {
			self.seen_ids.insert(id);
			self.results.push(manga);
			true
		} else {
			false
		}
	}

	/// Add a manga, parsing its key as i64. Returns true if added.
	pub fn add(&mut self, manga: Manga) -> bool {
		if let Ok(id) = manga.key.parse::<i64>() {
			self.try_add(id, manga)
		} else {
			false
		}
	}

	/// Extend with an iterator of Manga, deduplicating by parsed key.
	pub fn extend<I: IntoIterator<Item = Manga>>(&mut self, iter: I) {
		for manga in iter {
			self.add(manga);
		}
	}

	/// Extend with items directly, marking IDs as seen but NOT checking for duplicates.
	/// Use this for "trusted" sources (like hidden content) that handle their own dedup.
	pub fn extend_unchecked<I: IntoIterator<Item = Manga>>(&mut self, iter: I) {
		for manga in iter {
			if let Ok(id) = manga.key.parse::<i64>()
				&& id > 0
			{
				self.seen_ids.insert(id);
			}
			self.results.push(manga);
		}
	}

	/// Consume and return the deduplicated results.
	pub fn finish(self) -> Vec<Manga> {
		self.results
	}

	/// Get current result count.
	pub fn len(&self) -> usize {
		self.results.len()
	}

	/// Check if results are empty.
	pub fn is_empty(&self) -> bool {
		self.results.is_empty()
	}
}

// === Public API ===

pub fn search_by_keyword(keyword: &str, page: i32) -> Result<MangaPageResult> {
	if keyword.trim().is_empty() {
		return Ok(MangaPageResult::default());
	}

	// URL/ID Parsing (Direct Access)
	// If the keyword is a numeric ID or a recognized URL, try to fetch it directly.
	// This bypasses the Search API which might hide some content.
	if page == 1
		&& let Some(id) = parse_manga_id(keyword)
		&& let Some(manga) = fetch_manga_by_id(&id)
	{
		return Ok(MangaPageResult {
			entries: vec![manga],
			has_next_page: false,
		});
	}

	let mut token = settings::get_current_token();
	let mut search_data: Option<models::SearchData> = None;

	// Search API Request with Retry
	for retry_count in 0..2 {
		let token_ref = token.as_deref();
		let search_url = net::urls::search(keyword, page);
		let request = net::auth_request(&search_url, token_ref)
			.unwrap_or_else(|_| net::get_request(&search_url).expect("Invalid URL"));

		if let Ok(response) = request.send()
			&& let Ok(api_resp) = response.get_json_owned::<models::ApiResponse<models::SearchData>>()
		{
			if api_resp.errno.unwrap_or(0) == 99 {
				// Token expired, try refresh
				if retry_count == 0
					&& let Ok(Some(new_token)) = net::try_refresh_token()
				{
					token = Some(new_token);
					continue;
				}
				break;
			}
			search_data = api_resp.data;
			break;
		}
	}

	// Result Merging
	let mut results: Vec<Manga> = if let Some(data) = search_data {
		data.list.into_iter().map(Into::into).collect()
	} else {
		Vec::new()
	};

	let mut seen_ids: HashSet<i64> = results
		.iter()
		.filter_map(|m| m.key.parse::<i64>().ok())
		.collect();

	// Hidden Content Scan (if enabled)
	let mut hidden_has_next = false;
	if settings::deep_search_enabled() {
		let hidden_manga = scan_hidden_content(
			page,
			keyword,
			&mut seen_ids,
			token.as_deref(),
			true,   // match_by_name: true for keyword search
			false,  // strict_mode: not applicable for name matching
		);
		if !hidden_manga.is_empty() {
			hidden_has_next = true;
			results.extend(hidden_manga);
		}
	}

	let has_next_page = !results.is_empty() || hidden_has_next;

	Ok(MangaPageResult {
		entries: results,
		has_next_page,
	})
}

pub fn search_by_author(author: &str, page: i32) -> Result<MangaPageResult> {
	if author.trim().is_empty() {
		return Ok(MangaPageResult::default());
	}
	let mut strict_ids: Vec<i64> = Vec::new();
	let mut partial_ids: Vec<i64> = Vec::new();
	let mut seen_authors: Vec<String> = Vec::new();
	let mut strict_manga: Vec<Manga> = Vec::new();

	let token = settings::get_current_token();
	let token_ref = token.as_deref();

	// Direct Search
	let mut keyword_manga =
		collect_tags_from_search(author, author, &mut seen_authors, &mut strict_ids, &mut partial_ids, &mut strict_manga, token_ref)?;

	// Global Strict Priority Decision
	let exact_match_found = !strict_ids.is_empty();

	let direct_ids = if exact_match_found {
		strict_ids
	} else {
		partial_ids // Fallback to partials if NO strict found
	};

	// Fuzzy Fallback (only if NO tags found at all)
	let final_tag_ids = if direct_ids.is_empty() && keyword_manga.is_empty() {
		let mut fuzzy_strict = Vec::new();
		let mut fuzzy_partial = Vec::new();
		let fuzzy_manga = try_fuzzy_author_search(author, &mut seen_authors, &mut fuzzy_strict, &mut fuzzy_partial, &mut strict_manga, token_ref)?;
		
		keyword_manga.extend(fuzzy_manga);
		
		if !fuzzy_strict.is_empty() {
			fuzzy_strict
		} else {
			fuzzy_partial
		}
	} else {
		direct_ids
	};

	// Tag Search & Merge
	let (tag_manga, tag_total) = fetch_manga_by_tags(&final_tag_ids, page, token_ref)?;

	let mut merger = MangaMerger::new();

	if settings::deep_search_enabled() {
		let mut hidden_seen: HashSet<i64> = HashSet::new();
		let hidden_manga = scan_hidden_content(page, author, &mut hidden_seen, token_ref, false, exact_match_found);
		merger.extend_unchecked(hidden_manga);
	}

	// Add results based on Strict Priority
	let tag_iter = tag_manga.into_iter().map(|i| -> Manga { i.into() });

	// Result Merging:
	// If strict matches exist, enable "Strict Mode":
	// 1. Only include results that were strictly matched (ID-based or Name-based).
	// 2. Discard fuzzy candidates from the initial keyword search to eliminate noise.

	if exact_match_found {
		// Strict Mode: Use Token/Tag results AND Verified Strict Candidates.
		merger.extend(tag_iter.chain(strict_manga));
	} else {
		// Fallback/Fuzzy Mode: Merge everything.
		let keyword_iter = keyword_manga.into_iter().map(|i| -> Manga { i.into() });
		merger.extend(tag_iter.chain(keyword_iter));
	}

	if !merger.is_empty() {
		let has_next = if tag_total > 0 {
			tag_total >= 100
		} else {
			merger.len() >= 100
		};
		return Ok(MangaPageResult {
			entries: merger.finish(),
			has_next_page: has_next,
		});
	}

	Ok(MangaPageResult::default())
}

// === Private Types ===

#[derive(PartialEq, Default)]
enum MatchResult {
	Strict,
	Partial,
	#[default]
	None,
}

#[derive(Default)]
struct AuthorMatchResult {
	strict_ids: Vec<i64>,
	partial_ids: Vec<i64>,
	match_type: MatchResult,
}

// === Private Helpers ===

/// Parse manga ID from keyword (numeric ID or URL)
fn parse_manga_id(keyword: &str) -> Option<String> {
	let trimmed = keyword.trim();
	
	// Direct numeric ID
	if trimmed.chars().all(|c| c.is_ascii_digit()) && !trimmed.is_empty() {
		return Some(trimmed.to_string());
	}
	
	// URL patterns: /details/ID or /comic/ID
	if trimmed.contains("zaimanhua.com") {
		// Extract ID from URL like https://manhua.zaimanhua.com/details/70258
		if let Some(pos) = trimmed.rfind('/') {
			let after = &trimmed[pos + 1..];
			let id_part: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
			if !id_part.is_empty() {
				return Some(id_part);
			}
		}
	}
	
	None
}

/// Fetch manga directly by ID (bypasses search API)
fn fetch_manga_by_id(id: &str) -> Option<Manga> {
	let url = net::urls::detail(id.parse::<i64>().unwrap_or(0));
	let token = settings::get_current_token();
	
	let response: models::ApiResponse<models::DetailData> = 
		net::auth_request(&url, token.as_deref()).ok()?.json_owned().ok()?;
	
	if response.errno.unwrap_or(0) != 0 {
		return None;
	}
	
	let detail = response.data?.data?;
	Some(detail.into_manga(id.to_string()))
}

fn collect_tags_from_search(
	keyword: &str,
	target_author: &str,
	seen_authors: &mut Vec<String>,
	strict_ids: &mut Vec<i64>,
	partial_ids: &mut Vec<i64>,
	strict_manga: &mut Vec<Manga>,
	token: Option<&str>,
) -> Result<Vec<models::SearchItem>> {
	let mut matched_items = Vec::new();

	let url = net::urls::search_sized(&keyword.replace('&', " "), 1, 100);

	if let Ok(response) = net::send_authed_request::<models::SearchData>(&url, token)
		&& let Some(data) = response.data
	{
		// Identify Candidate Items locally
		let candidates: Vec<&models::SearchItem> = data.list
			.iter()
			.filter(|item| item.matches_author(target_author))
			.collect();

		if candidates.is_empty() {
			return Ok(matched_items);
		}

		// Batch Verify Candidates
		let requests: Vec<Request> = candidates
			.iter()
			.map(|item| {
				let url = net::urls::detail(item.id);
				net::auth_request(&url, token)
					.unwrap_or_else(|_| net::get_request(&url).expect("Invalid URL"))
			})
			.collect();

		let responses = Request::send_all(requests);

		// Process Responses
		for (i, response_result) in responses.into_iter().enumerate() {
			let candidate = candidates[i];
			let manga_authors = candidate.authors.as_deref().unwrap_or("");
			let author_key = manga_authors.to_string();

			// Ensure "seen_authors" constraint logic is respected
			if seen_authors.contains(&author_key) {
				matched_items.push(candidate.clone());
				continue;
			}

			// Validate response
			if let Ok(resp) = response_result
				&& let Ok(api_resp) = resp.get_json_owned::<models::ApiResponse<models::DetailData>>()
				&& let Some(data_root) = api_resp.data
				&& let Some(detail) = data_root.data
			{
				seen_authors.push(author_key);

				// Use pure logic extractor (returns new struct)
				let result = extract_author_tags_from_detail(&detail, target_author);
				strict_ids.extend(result.strict_ids);
				partial_ids.extend(result.partial_ids);

				match result.match_type {
					MatchResult::Strict => {
						matched_items.push(candidate.clone());
						strict_manga.push(candidate.clone().into());
					},
					MatchResult::Partial => {
						matched_items.push(candidate.clone());
					},
					MatchResult::None => {}
				}
			}
		}
	}
	Ok(matched_items)
}

fn try_fuzzy_author_search(
	author: &str,
	seen_authors: &mut Vec<String>,
	strict_ids: &mut Vec<i64>,
	partial_ids: &mut Vec<i64>,
	strict_manga: &mut Vec<Manga>,
	token: Option<&str>,
) -> Result<Vec<models::SearchItem>> {
	let core_name = author;
	let short_core = if core_name.chars().count() >= 4 {
		core_name.chars().take(2).collect::<String>()
	} else {
		core_name.to_string()
	};

	let mut matched_items = Vec::new();

	for core in [core_name, short_core.as_str()] {
		if core.is_empty() || core == author || (!strict_ids.is_empty() || !partial_ids.is_empty()) {
			continue;
		}

		let keywords = collect_tags_from_search(core, author, seen_authors, strict_ids, partial_ids, strict_manga, token)?;
		matched_items.extend(keywords);

		if !strict_ids.is_empty() || !partial_ids.is_empty() {
			break;
		}
	}
	Ok(matched_items)
}

fn fetch_manga_by_tags(tag_ids: &[i64], page: i32, token: Option<&str>) -> Result<(Vec<models::FilterItem>, i32)> {
	if tag_ids.is_empty() {
		return Ok((Vec::new(), 0));
	}

	let tag_requests: Vec<_> = tag_ids
		.iter()
		.filter_map(|tid| {
			let url = net::urls::filter_theme(*tid, page);
			net::auth_request(&url, token).ok()
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
	Ok((items, total))
}

fn scan_hidden_content(
	page: i32,
	target: &str,
	seen_ids: &mut HashSet<i64>,
	token: Option<&str>,
	match_by_name: bool,
	strict_mode: bool,
) -> Vec<Manga> {
	let mut found_manga = Vec::new();
	let hidden_start_page = (page - 1) * 5 + 1;
	let scanner = net::HiddenContentScanner::new(hidden_start_page, 3, token);
	let target_lower = target.to_lowercase();

	for items in scanner {
		let mut batch_found_any = false;
		for item in items {
			let is_match = if match_by_name {
				// Name OR author contains target (for keyword search)
				let name_lower = item.name.to_lowercase();
				let auth_lower = item.authors.as_deref().unwrap_or("").to_lowercase();
				name_lower.contains(&target_lower) || auth_lower.contains(&target_lower)
			} else if strict_mode {
				// Exact author match (Case Insensitive)
				item.authors.as_ref()
					.map(|a| a.split(',').any(|s| s.trim().eq_ignore_ascii_case(target)))
					.unwrap_or(false)
			} else {
				// Fuzzy author match
				item.matches_author(target)
			};

			if is_match && !seen_ids.contains(&item.id) {
				seen_ids.insert(item.id);
				found_manga.push(item.into());
				batch_found_any = true;
			}
		}

		if batch_found_any {
			break;
		}
	}
	found_manga
}

/// Pure function: extracts author tag IDs from manga detail.
fn extract_author_tags_from_detail(
	detail: &models::MangaDetail,
	target_author: &str,
) -> AuthorMatchResult {
	let Some(authors) = &detail.authors else {
		return AuthorMatchResult::default();
	};

	let mut seen_strict: HashSet<i64> = HashSet::new();
	let mut seen_partial: HashSet<i64> = HashSet::new();

	// Single-pass: collect strict and partial matches
	let (strict_ids, partial_ids): (Vec<i64>, Vec<i64>) = authors
		.iter()
		.filter_map(|author| {
			let name = author.tag_name.as_ref()?;
			let tid = author.tag_id.filter(|&id| id > 0)?;
			if name.trim().is_empty() {
				return None;
			}

			let name_trimmed = name.trim();
			let target_trimmed = target_author.trim();
			let target_lower = target_trimmed.to_lowercase();

			if name_trimmed.eq_ignore_ascii_case(target_trimmed) && seen_strict.insert(tid) {
				return Some((Some(tid), None)); // Strict match
			} else if (name_trimmed.to_lowercase().contains(&target_lower)
				|| target_lower.contains(&name_trimmed.to_lowercase()))
				&& seen_partial.insert(tid)
			{
				return Some((None, Some(tid))); // Partial match
			}
			None
		})
		.fold((Vec::new(), Vec::new()), |(mut s, mut p), (strict, partial)| {
			if let Some(id) = strict { s.push(id); }
			if let Some(id) = partial { p.push(id); }
			(s, p)
		});

	let match_type = if !strict_ids.is_empty() {
		MatchResult::Strict
	} else if !partial_ids.is_empty() {
		MatchResult::Partial
	} else {
		MatchResult::None
	};

	AuthorMatchResult { strict_ids, partial_ids, match_type }
}
