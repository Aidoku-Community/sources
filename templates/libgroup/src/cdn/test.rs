use super::*;
use aidoku::alloc::{String, collections::btree_map::BTreeMap, string::ToString};
use aidoku_test::aidoku_test;

fn test_context() -> Context {
	Context {
		api_url: "http://fake.api".to_string(),
		base_url: "http://fake.base".to_string(),
		site_id: 1,
		cover_quality: "high".to_string(),
	}
}

fn mock_now() -> i64 {
	1_000_000
}

fn make_cache_with_ttl(ttl: i64) -> ImageServerCache {
	ImageServerCache::new_with_ttl(ttl, mock_now)
}

#[aidoku_test]
fn cache_hit_returns_same_url() {
	let cache = make_cache_with_ttl(3600);
	let mut servers = BTreeMap::new();
	let mut inner = BTreeMap::new();
	inner.insert("server1".to_string(), "http://img.server/1".to_string());
	servers.insert(1u8, inner);

	{
		let mut guard = cache.cache.write();
		*guard = Some(CacheEntry::new(servers.clone(), mock_now()));
	}

	let url = cache.extract_url(&servers, &1u8, "server1");
	assert_eq!(url, "http://img.server/1");
}

#[aidoku_test]
fn expired_entry_detected() {
	let ctx = test_context();
	let cache = make_cache_with_ttl(1); // 1 second TTL

	let mut servers = BTreeMap::new();
	let mut inner = BTreeMap::new();
	inner.insert("server1".to_string(), "http://img.server/1".to_string());
	servers.insert(1u8, inner);

	// Seed with time 10s in past (expired)
	{
		let mut guard = cache.cache.write();
		*guard = Some(CacheEntry::new(servers.clone(), mock_now() - 10));
	}

	// Should not panic, should try network, fail, and handle safely
	let _ = cache.get_base_url(&ctx);
}

#[aidoku_test]
fn extract_url_returns_empty_when_not_found() {
	let cache = make_cache_with_ttl(3600);
	let servers: BTreeMap<u8, BTreeMap<String, String>> = BTreeMap::new();
	let url = cache.extract_url(&servers, &99u8, "server1");
	assert_eq!(url, "");
}
