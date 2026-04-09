use crate::{models, net, settings};
use aidoku::{
	Manga, MangaPageResult, Result,
	alloc::{String, Vec, string::ToString, vec},
	imports::net::Request,
	prelude::error,
};
use hashbrown::HashSet;

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

	fn try_add(&mut self, id: i64, manga: Manga) -> bool {
		if id > 0 && !self.seen_ids.contains(&id) {
			self.seen_ids.insert(id);
			self.results.push(manga);
			true
		} else {
			false
		}
	}

	pub fn add(&mut self, manga: Manga) -> bool {
		if let Ok(id) = manga.key.parse::<i64>() {
			self.try_add(id, manga)
		} else {
			false
		}
	}

	pub fn extend<I: IntoIterator<Item = Manga>>(&mut self, iter: I) {
		for manga in iter {
			self.add(manga);
		}
	}

	pub fn finish(self) -> Vec<Manga> {
		self.results
	}

	pub fn len(&self) -> usize {
		self.results.len()
	}

	pub fn is_empty(&self) -> bool {
		self.results.is_empty()
	}
}

pub fn search_by_keyword(keyword: &str, page: i32) -> Result<MangaPageResult> {
	if keyword.trim().is_empty() {
		return Ok(MangaPageResult::default());
	}

	if page == 1
		&& let Some(id) = parse_manga_id(keyword)
		&& let Some(manga) = fetch_manga_by_id(&id)
	{
		return Ok(MangaPageResult {
			entries: vec![manga],
			has_next_page: false,
		});
	}

	let token = settings::get_token();
	let search_url = net::urls::search(keyword, page);
	let entries: Vec<Manga> =
		net::send_authed_request::<models::SearchData>(&search_url, token.as_deref())
			.ok()
			.and_then(|r| r.data)
			.map(|d| d.list.into_iter().map(Into::into).collect())
			.unwrap_or_default();

	let has_next_page = !entries.is_empty();

	Ok(MangaPageResult {
		entries,
		has_next_page,
	})
}

pub fn search_by_author(author: &str, page: i32) -> Result<MangaPageResult> {
	if author.trim().is_empty() {
		return Ok(MangaPageResult::default());
	}
	let mut strict_ids: Vec<i64> = Vec::new();
	let mut partial_ids: Vec<i64> = Vec::new();
	let mut seen_manga_ids: Vec<i64> = Vec::new();
	let mut strict_manga: Vec<Manga> = Vec::new();

	let token = settings::get_token();
	let token_ref = token.as_deref();

	let mut keyword_manga = collect_tags_from_search(
		author,
		author,
		&mut seen_manga_ids,
		&mut strict_ids,
		&mut partial_ids,
		&mut strict_manga,
		token_ref,
	)?;

	let exact_match_found = !strict_ids.is_empty();

	let direct_ids = if exact_match_found {
		strict_ids
	} else {
		partial_ids
	};

	let final_tag_ids = if direct_ids.is_empty() && keyword_manga.is_empty() {
		let mut fuzzy_strict = Vec::new();
		let mut fuzzy_partial = Vec::new();
		let fuzzy_manga = try_fuzzy_author_search(
			author,
			&mut seen_manga_ids,
			&mut fuzzy_strict,
			&mut fuzzy_partial,
			&mut strict_manga,
			token_ref,
		)?;

		keyword_manga.extend(fuzzy_manga);

		if !fuzzy_strict.is_empty() {
			fuzzy_strict
		} else {
			fuzzy_partial
		}
	} else {
		direct_ids
	};

	let (tag_manga, tag_total) = fetch_manga_by_tags(&final_tag_ids, page, token_ref)?;

	let mut merger = MangaMerger::new();
	let tag_iter = tag_manga.into_iter().map(Into::<Manga>::into);

	if exact_match_found {
		merger.extend(tag_iter.chain(strict_manga));
	} else {
		let keyword_iter = keyword_manga.into_iter().map(Into::<Manga>::into);
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

fn parse_manga_id(keyword: &str) -> Option<String> {
	let trimmed = keyword.trim();

	if trimmed.chars().all(|c| c.is_ascii_digit()) && !trimmed.is_empty() {
		return Some(trimmed.to_string());
	}

	if trimmed.contains("zaimanhua.com")
		&& let Some(pos) = trimmed.rfind('/')
	{
		let after = &trimmed[pos + 1..];
		let id_part: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
		if !id_part.is_empty() {
			return Some(id_part);
		}
	}

	None
}

fn fetch_manga_by_id(id: &str) -> Option<Manga> {
	let url = net::urls::detail(id.parse::<i64>().unwrap_or(0));
	let token = settings::get_token();

	let response: models::ApiResponse<models::DetailData> =
		net::auth_request(&url, token.as_deref())
			.ok()?
			.json_owned()
			.ok()?;

	if response.errno.unwrap_or(0) != 0 {
		return None;
	}

	let detail = response.data?.data?;
	Some(detail.into_manga(id.to_string()))
}

fn collect_tags_from_search(
	keyword: &str,
	target_author: &str,
	seen_manga_ids: &mut Vec<i64>,
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
		let candidates: Vec<&models::SearchItem> = data
			.list
			.iter()
			.filter(|item| item.matches_author(target_author))
			.collect();

		if candidates.is_empty() {
			return Ok(matched_items);
		}

		let requests: Vec<Request> = candidates
			.iter()
			.map(|item| {
				let url = net::urls::detail(item.id);
				net::auth_request(&url, token)
			})
			.collect::<Result<Vec<_>>>()?;

		let responses = Request::send_all(requests);

		for (response_result, candidate) in responses.into_iter().zip(candidates) {
			if seen_manga_ids.contains(&candidate.id) {
				matched_items.push(candidate.clone());
				continue;
			}

			if let Ok(resp) = response_result
				&& let Ok(api_resp) =
					resp.get_json_owned::<models::ApiResponse<models::DetailData>>()
				&& let Some(data_root) = api_resp.data
				&& let Some(detail) = data_root.data
			{
				seen_manga_ids.push(candidate.id);

				let result = extract_author_tags_from_detail(&detail, target_author);
				strict_ids.extend(result.strict_ids);
				partial_ids.extend(result.partial_ids);

				match result.match_type {
					MatchResult::Strict => {
						matched_items.push(candidate.clone());
						strict_manga.push(candidate.clone().into());
					}
					MatchResult::Partial => {
						matched_items.push(candidate.clone());
					}
					MatchResult::None => {}
				}
			}
		}
	}
	Ok(matched_items)
}

fn try_fuzzy_author_search(
	author: &str,
	seen_manga_ids: &mut Vec<i64>,
	strict_ids: &mut Vec<i64>,
	partial_ids: &mut Vec<i64>,
	strict_manga: &mut Vec<Manga>,
	token: Option<&str>,
) -> Result<Vec<models::SearchItem>> {
	if author.chars().count() < 4 || !strict_ids.is_empty() || !partial_ids.is_empty() {
		return Ok(Vec::new());
	}

	let short_core: String = author.chars().take(2).collect();

	collect_tags_from_search(
		&short_core,
		author,
		seen_manga_ids,
		strict_ids,
		partial_ids,
		strict_manga,
		token,
	)
}

fn fetch_manga_by_tags(
	tag_ids: &[i64],
	page: i32,
	token: Option<&str>,
) -> Result<(Vec<models::FilterItem>, i32)> {
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

fn extract_author_tags_from_detail(
	detail: &models::MangaDetail,
	target_author: &str,
) -> AuthorMatchResult {
	let Some(authors) = &detail.authors else {
		return AuthorMatchResult::default();
	};

	let target_trimmed = target_author.trim();
	let target_lower = target_trimmed.to_lowercase();

	let mut strict_ids = Vec::with_capacity(2);
	let mut partial_ids = Vec::with_capacity(2);

	for author in authors {
		let Some(name) = author.tag_name.as_ref() else {
			continue;
		};
		let Some(tid) = author.tag_id.filter(|&id| id > 0) else {
			continue;
		};

		let name_trimmed = name.trim();
		if name_trimmed.is_empty() {
			continue;
		}

		if name_trimmed.eq_ignore_ascii_case(target_trimmed) {
			if !strict_ids.contains(&tid) {
				strict_ids.push(tid);
			}
		} else {
			let name_lower = name_trimmed.to_lowercase();
			if (name_lower.contains(&target_lower) || target_lower.contains(&name_lower))
				&& !partial_ids.contains(&tid)
			{
				partial_ids.push(tid);
			}
		}
	}

	let match_type = if !strict_ids.is_empty() {
		MatchResult::Strict
	} else if !partial_ids.is_empty() {
		MatchResult::Partial
	} else {
		MatchResult::None
	};

	AuthorMatchResult {
		strict_ids,
		partial_ids,
		match_type,
	}
}

pub fn resolve_theme_id(name: &str) -> Result<String> {
	let url = net::urls::classify();
	let response: models::ApiResponse<models::ClassifyData> =
		net::get_request(&url)?.json_owned()?;
	let data = response.data.ok_or_else(|| error!("分类数据缺失"))?;

	let target = models::normalize_tag_name(name.to_string());

	data.classify_list
		.into_iter()
		.find(|g| g.id == 1)
		.and_then(|g| {
			g.list.into_iter().find_map(|t| {
				(models::normalize_tag_name(t.tag_name) == target).then(|| t.tag_id.to_string())
			})
		})
		.ok_or_else(|| error!("未找到标签: {name}"))
}
