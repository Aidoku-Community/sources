use crate::{
	Kind,
	models::{ClassifyTag, ListData, UnifiedData, tag_name},
	net::{self, urls},
	settings,
};
use aidoku::{
	MangaPageResult, Result,
	alloc::{String, Vec},
	imports::net::Request,
	prelude::*,
};
use core::cell::RefCell;

pub fn search(kind: Kind, keyword: &str, page: i32) -> Result<MangaPageResult> {
	let token = settings::get_token();
	let token = token.as_deref();
	// search/index doesn't match ids, and search/unified reads a number only as an id
	let id = keyword.parse::<u32>().ok().filter(|_| page == 1);
	let requests = [
		Some(net::request(&urls::search(kind, keyword, page), token)),
		id.map(|id| net::request(&urls::search_id(kind, id), token)),
	]
	.into_iter()
	.flatten()
	.collect::<Result<Vec<_>>>()?;
	let mut responses = Request::send_all(requests).into_iter();
	let mut result = net::parse::<ListData>(responses.next())?.into_page_result(page);
	if let Some(manga) = id.and_then(|id| {
		net::parse::<UnifiedData>(responses.next())
			.ok()?
			.into_manga(id)
	}) {
		result.entries.retain(|entry| entry.key != manga.key);
		result.entries.insert(0, manga);
	}
	Ok(result)
}

pub fn search_key(kind: Kind, key: &str) -> Result<MangaPageResult> {
	let token = settings::get_token();
	Ok(MangaPageResult {
		entries: net::fetch::<UnifiedData>(&urls::search_id(kind, key), token.as_deref())?
			.into_manga(key)
			.into_iter()
			.collect(),
		has_next_page: false,
	})
}

pub fn list(kind: Kind, query: &str, page: i32) -> Result<MangaPageResult> {
	Ok(fetch_list(&urls::list(kind, query, page))?.into_page_result(page))
}

pub fn fetch_list(url: &str) -> Result<ListData> {
	net::fetch(url, settings::get_token().as_deref())
}

pub fn subscriptions(kind: Kind, page: i32) -> Result<MangaPageResult> {
	let url = urls::subscriptions(kind, page);
	let list = match settings::get_token() {
		Some(token) => net::send_authed::<ListData>(&url, &token)?.into_result()?,
		None => net::fetch(&url, None)?,
	};
	Ok(list.into_page_result(page))
}

pub fn tag_id(
	cache: &RefCell<Option<(String, i64)>>,
	name: &str,
	tags: impl FnOnce() -> Result<Vec<ClassifyTag>>,
) -> Result<i64> {
	if let Some((cached, id)) = &*cache.borrow()
		&& cached == name
	{
		return Ok(*id);
	}
	let id = tags()?
		.into_iter()
		.find_map(|tag| (tag_name(tag.tag_name) == name).then_some(tag.tag_id))
		.ok_or_else(|| error!("未找到标签：{name}"))?;
	cache.replace(Some((name.into(), id)));
	Ok(id)
}

pub struct SiteLink<'a> {
	pub segments: Vec<&'a str>,
	query: &'a str,
}

impl<'a> SiteLink<'a> {
	pub fn parse(url: &'a str) -> Option<Self> {
		let (_, path) = url.split_once("zaimanhua.com/")?;
		let path = path.split_once('#').map_or(path, |(path, _)| path);
		let (path, query) = path.split_once('?').unwrap_or((path, ""));
		Some(Self {
			segments: path.split('/').filter(|s| !s.is_empty()).collect(),
			query,
		})
	}

	pub fn param(&self, name: &str) -> Option<&'a str> {
		self.query
			.split('&')
			.find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
			.filter(|value| is_id(value))
	}
}

pub fn is_id(value: &str) -> bool {
	!value.is_empty() && value.bytes().all(|b| b.is_ascii_digit())
}
