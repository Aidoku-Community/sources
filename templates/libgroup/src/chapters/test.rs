use crate::chapters::{ChaptersCache, TimedVec};
use crate::models::chapter::LibGroupChapterListItem;
use aidoku::alloc::{string::ToString, vec};

fn fake_now() -> i64 {
	1_000_000
}

fn make_cache_with_ttl(ttl: Option<i64>) -> ChaptersCache {
	ChaptersCache::new_with_ttl(ttl, fake_now)
}

fn make_item(id: &str) -> LibGroupChapterListItem {
	LibGroupChapterListItem {
		volume: "1".to_string(),
		number: id.to_string(),
		name: Some("Test Chapter".to_string()),
		branches: vec![],
	}
}

#[test]
fn cache_hit_returns_same_data() {
	let cache = make_cache_with_ttl(None);
	let manga_key = "manga1";

	{
		let mut guard = cache.cache.write();
		guard.insert(
			manga_key.to_string(),
			TimedVec::new(vec![make_item("ch1")], fake_now()),
		);
	}

	let chapters = cache.get_chapters(manga_key, "http://fake.base");
	assert!(chapters.is_ok());
	let chs = chapters.unwrap();
	assert_eq!(chs.len(), 1);
	assert_eq!(chs[0].number, "ch1");
}

#[test]
fn ttl_expiration_detected() {
	static mut CURRENT_TIME: i64 = 1_000_000;
	let cache = ChaptersCache::new_with_ttl(Some(10), || unsafe { CURRENT_TIME });

	let manga_key = "manga2";
	unsafe { CURRENT_TIME = 1_000_000 };
	{
		let mut guard = cache.cache.write();
		guard.insert(
			manga_key.to_string(),
			TimedVec::new(vec![make_item("old")], unsafe { CURRENT_TIME }),
		);
	}

	unsafe { CURRENT_TIME += 20 };

	let guard = cache.cache.read();
	let expired = guard
		.get(manga_key)
		.unwrap()
		.is_expired(unsafe { CURRENT_TIME }, Some(10));
	assert!(expired);
}

#[test]
fn clear_removes_all_entries() {
	let cache = make_cache_with_ttl(None);
	let manga_key = "manga3";

	{
		let mut guard = cache.cache.write();
		guard.insert(
			manga_key.to_string(),
			TimedVec::new(vec![make_item("ch1")], fake_now()),
		);
	}

	cache.clear();
	assert!(cache.cache.read().is_empty());
}
