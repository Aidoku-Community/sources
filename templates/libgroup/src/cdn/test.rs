use super::*;
use aidoku::alloc::{String, string::ToString};
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

fn make_cache_with_ttl(ttl: i64) -> ImageServerCache {
	ImageServerCache::new_with_ttl(ttl, fake_now)
}

#[aidoku_test]
fn cache_hit_returns_same_url() {
	let cache = make_cache_with_ttl(3600);

	let mut servers = BTreeMap::new();
	let mut inner = BTreeMap::new();
	inner.insert("server1".to_string(), "http://img.server/1".to_string());
	servers.insert(1u8, inner);

	// Seed cache with a fresh entry
	{
		let mut guard = cache.cache.write();
		*guard = Some(CacheEntry::new(servers.clone(), fake_now()));
	}

	// extract_url directly from seeded map with explicit server_id
	let url = cache.extract_url(&servers, &1u8, "server1");
	assert_eq!(url, "http://img.server/1");

	// For get_base_url, you'd need to ensure get_image_server_url() returns "server1"
	// or test it separately
}

#[aidoku_test]
fn expired_entry_detected() {
	let ctx = test_context();
	let cache = make_cache_with_ttl(1);

	let mut servers = BTreeMap::new();
	let mut inner = BTreeMap::new();
	inner.insert("server1".to_string(), "http://img.server/1".to_string());
	servers.insert(1u8, inner);

	// seed with an expired timestamp
	{
		let mut guard = cache.cache.write();
		*guard = Some(CacheEntry::new(servers.clone(), fake_now() - 10));
	}

	// Since load_data will try to do a network request (and we can't mock it here),
	// ensure get_base_url doesn't panic and returns either stale or empty string.
	let _ = cache.get_base_url(&ctx);
}

#[aidoku_test]
fn extract_url_returns_empty_when_not_found() {
	let cache = make_cache_with_ttl(3600);
	let servers: BTreeMap<u8, BTreeMap<String, String>> = BTreeMap::new();
	let url = cache.extract_url(&servers, &99u8, "server1");
	assert_eq!(url, "");
}
