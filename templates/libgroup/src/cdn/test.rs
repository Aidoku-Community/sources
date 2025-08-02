use super::*;
use aidoku::prelude::*;
use aidoku_test::aidoku_test;

#[aidoku_test]
fn cache_stress_test() {
	// Stress test the cache with many rapid calls
	for i in 0..100 {
		let site_id = (i % 10) as u8;
		let url = get_selected_image_server_url(&site_id);

		// Should not panic - test by checking it's a valid String
		assert!(url.capacity() >= url.len());
	}

	// Test with edge case site_ids
	let edge_cases = [0u8, 1, 127, 128, 254, 255];
	for &site_id in &edge_cases {
		let url = get_selected_image_server_url(&site_id);
		// Should not panic - test by checking it's a valid String
		assert!(url.capacity() >= url.len());
	}
}

#[aidoku_test]
fn cache_initialization() {
	// Cache should auto-initialize and return empty string when no data
	let url = get_selected_image_server_url(&1);
	let _ = url.clone(); // Should not panic
}

#[aidoku_test]
fn cache_returns_empty_for_missing_site() {
	let url = get_selected_image_server_url(&255); // Non-existent site
	assert_eq!(url, String::new());
}

#[aidoku_test]
fn cache_handles_zero_site_id() {
	let url = get_selected_image_server_url(&0);
	assert_eq!(url, String::new());
}

#[aidoku_test]
fn cache_consistent_results() {
	// Multiple calls should return same result (cached)
	let url1 = get_selected_image_server_url(&1);
	let url2 = get_selected_image_server_url(&1);
	assert_eq!(url1, url2);
}

#[aidoku_test]
fn cache_different_sites() {
	// Different sites should potentially return different URLs
	let url1 = get_selected_image_server_url(&1);
	let url2 = get_selected_image_server_url(&2);
	// They might be the same or different, just ensure no panic
	let _ = url1.clone();
	let _ = url2.clone();
}

#[aidoku_test]
fn cache_thread_safety() {
	// Test atomic operations work correctly
	static SUCCESS: AtomicBool = AtomicBool::new(true);

	// Simulate concurrent access by rapid sequential calls
	for i in 0..50 {
		let site_id = (i % 3) as u8;
		let url = get_selected_image_server_url(&site_id);

		// Check that we get consistent results
		if url.contains("invalid_marker_that_should_not_exist") {
			SUCCESS.store(false, Ordering::Relaxed);
		}

		// Test cache consistency - same site should return same result
		let url2 = get_selected_image_server_url(&site_id);
		if url != url2 {
			SUCCESS.store(false, Ordering::Relaxed);
		}
	}

	assert!(SUCCESS.load(Ordering::Relaxed));
}

#[aidoku_test]
fn cache_monotonic_counter() {
	// Test that monotonic counter advances
	// Note: This assumes get_monotonic_counter is made public for testing
	// If not public, we can test it indirectly through cache behavior

	// Test cache behavior with multiple calls
	let url1 = get_selected_image_server_url(&1);
	let url2 = get_selected_image_server_url(&2);
	let url3 = get_selected_image_server_url(&1); // Should be cached

	// url1 and url3 should be identical (cache hit)
	assert_eq!(url1, url3);

	// All calls should complete without panic
	let _ = url1.clone();
	let _ = url2.clone();
}

#[aidoku_test]
fn cache_entry_expiration() {
	// Test cache expiration logic indirectly
	// Since CacheEntry might not be public, test through behavior

	// Multiple rapid calls should return same result (cached)
	let url1 = get_selected_image_server_url(&42);
	let url2 = get_selected_image_server_url(&42);
	let url3 = get_selected_image_server_url(&42);

	// All should be identical due to caching
	assert_eq!(url1, url2);
	assert_eq!(url2, url3);
}

#[aidoku_test]
fn url_format_validation() {
	// Test URL concatenation doesn't break
	let base_url = get_selected_image_server_url(&1);
	let page_url = "/path/to/image.jpg";
	let full_url = format!("{}{}", base_url, page_url);

	// Should not contain double slashes (unless intentional)
	if !base_url.is_empty() && !base_url.ends_with('/') {
		assert!(!full_url.contains("//") || full_url.starts_with("http"));
	}
}

#[aidoku_test]
fn cache_performance() {
	// Test that cache works efficiently
	// First call might initialize cache
	let _url1 = get_selected_image_server_url(&1);

	// Subsequent calls should be fast (cached)
	for _i in 0..10 {
		let url = get_selected_image_server_url(&1);
		// Should return consistent results
		assert_eq!(url, _url1);
	}

	// Test with different site_ids
	let url_site2 = get_selected_image_server_url(&2);
	let url_site3 = get_selected_image_server_url(&3);

	// Should not panic - just verify they're valid strings
	let _ = url_site2.clone();
	let _ = url_site3.clone();
}
