use aidoku::{
	MangaPageResult, Page, PageContent, Result,
	alloc::{String, Vec, vec},
	bail,
	prelude::format,
};

use crate::{
	NOVEL_PREFIX, extract_id, markdown,
	models::{BlockState, ComicItem, NovelChapterResp, NovelResp, SearchResp, manga_list},
	net::{self, ApiContext},
	unserved,
};

pub fn listing(api: &ApiContext, block: &BlockState) -> Result<MangaPageResult> {
	let resp: SearchResp = api.get(net::url::novels())?;
	Ok(MangaPageResult {
		entries: manga_list(resp.content, &api.cdn_base, block, ComicItem::into_novel),
		has_next_page: false,
	})
}

pub fn key(url: &str) -> Option<String> {
	extract_id(url, "/novel/").map(|id| format!("{NOVEL_PREFIX}{id}"))
}

pub fn chapter_novel(api: &ApiContext, chapter_key: &str) -> Result<Option<String>> {
	let resp: NovelChapterResp = api.get(&net::url::novel_chapter(chapter_key))?;
	Ok(resp.novel_key())
}

pub fn link_key(api: &ApiContext, url: &str) -> Result<Option<String>> {
	match extract_id(url, "/novelchapters/") {
		Some(chapter_key) => chapter_novel(api, chapter_key),
		None => Ok(key(url)),
	}
}

fn fetch(api: &ApiContext, key: &str) -> Result<NovelResp> {
	api.get(&net::url::novel(
		key.strip_prefix(NOVEL_PREFIX).unwrap_or(key),
	))
}

pub fn visible(api: &ApiContext, key: &str, block: &BlockState) -> Result<NovelResp> {
	let resp = fetch(api, key)?;
	if resp.is_blocked(key, block) {
		bail!("此內容已被你封鎖")
	}
	Ok(resp)
}

pub fn direct_result(api: &ApiContext, key: &str, block: &BlockState) -> Result<MangaPageResult> {
	let resp = fetch(api, key)?;
	if resp.is_blocked(key, block) {
		return Ok(MangaPageResult::default());
	}
	Ok(MangaPageResult {
		entries: vec![resp.into_entry(key, &api.cdn_base)],
		has_next_page: false,
	})
}

pub fn pages(api: &ApiContext, key: &str, chapter_key: &str) -> Result<Vec<Page>> {
	let resp: NovelChapterResp = api.get(&net::url::novel_chapter(chapter_key))?;
	let text = markdown::from_html(resp.content.as_deref().unwrap_or_default(), &api.cdn_base);
	if text.is_empty() {
		return unserved(fetch(api, key)?.is_unpaid(chapter_key));
	}
	Ok(vec![Page {
		content: PageContent::text(text),
		..Default::default()
	}])
}
