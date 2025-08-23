use crate::chapters::{ChaptersCache, TimedVec};
use crate::context::Context;
use crate::models::chapter::LibGroupChapterListItem;
use aidoku::alloc::{string::ToString, vec};
use aidoku_test::aidoku_test;

fn test_context() -> Context {
	Context {
		api_url: "http://fake.api".to_string(),
		base_url: "http://fake.base".to_string(),
		site_id: 1,
		cover_quality: "high".to_string(),
	}
}

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

#[aidoku_test]
fn cache_hit_returns_same_data() {
	let ctx = test_context();
	let cache = make_cache_with_ttl(None);
	let manga_key = "manga1";

	let mut guard = cache.cache.write();
	guard.insert(
		manga_key.to_string(),
		TimedVec::new(vec![make_item("ch1")], fake_now()),
	);

	let chapters = cache.get_chapters(manga_key, &ctx);
	assert!(chapters.is_ok());
	let chs = chapters.unwrap();
	assert_eq!(chs.len(), 1);
	assert_eq!(chs[0].number, "ch1");
}

#[aidoku_test]
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

#[aidoku_test]
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
