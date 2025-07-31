use crate::auth::{get_token, refresh_token, set_token};
use aidoku::{
	alloc::{String, string::ToString},
	imports::defaults::{DefaultValue, defaults_set},
	prelude::*,
};
use aidoku_test::aidoku_test;
use serde_json::{from_str, to_string};

use crate::{
	auth::{AUTH_SCHEME, TOKEN_KEY},
	models::responses::TokenResponse,
};

// Test helper to create a valid token response JSON
fn create_test_token(access: Option<&str>, refresh: Option<&str>, expires: Option<i64>) -> String {
	let token = TokenResponse {
		access_token: access.map(String::from),
		refresh_token: refresh.map(String::from),
		expires_in: expires,
	};
	to_string(&token).unwrap_or_default()
}

// Test helper to clear stored token
fn clear_token() {
	defaults_set(TOKEN_KEY, DefaultValue::String(String::new()));
}

#[aidoku_test]
fn get_token_success() {
	// Setup: Store a valid token
	let token_json = create_test_token(Some("test_access"), Some("test_refresh"), Some(3600));
	set_token(token_json);

	// Test: Should successfully retrieve token
	let result = get_token();
	assert!(result.is_ok());

	let token = result.unwrap();
	assert_eq!(token.access_token, Some("test_access".to_string()));
	assert_eq!(token.refresh_token, Some("test_refresh".to_string()));
	assert_eq!(token.expires_in, Some(3600));

	// Cleanup
	clear_token();
}

#[aidoku_test]
fn get_token_no_token_stored() {
	// Ensure no token is stored
	clear_token();

	// Test: Should return error when no token exists
	let result = get_token();
	assert!(result.is_err());
}

#[aidoku_test]
fn get_token_invalid_json() {
	// Setup: Store invalid JSON
	defaults_set(TOKEN_KEY, DefaultValue::String("invalid_json".to_string()));

	// Test: Should return error for invalid JSON
	let result = get_token();
	assert!(result.is_err());

	// Cleanup
	clear_token();
}

#[aidoku_test]
fn set_token_stores_correctly() {
	let token_json = create_test_token(Some("stored_access"), Some("stored_refresh"), Some(7200));

	// Test: Store token
	set_token(token_json.clone());

	// Verify: Token should be retrievable
	let result = get_token();
	assert!(result.is_ok());

	let token = result.unwrap();
	assert_eq!(token.access_token, Some("stored_access".to_string()));
	assert_eq!(token.refresh_token, Some("stored_refresh".to_string()));
	assert_eq!(token.expires_in, Some(7200));

	// Cleanup
	clear_token();
}

#[aidoku_test]
fn set_token_overwrites_existing() {
	// Setup: Store initial token
	let initial_token =
		create_test_token(Some("initial_access"), Some("initial_refresh"), Some(1800));
	set_token(initial_token);

	// Test: Overwrite with new token
	let new_token = create_test_token(Some("new_access"), Some("new_refresh"), Some(3600));
	set_token(new_token);

	// Verify: Should retrieve the new token
	let result = get_token();
	assert!(result.is_ok());

	let token = result.unwrap();
	assert_eq!(token.access_token, Some("new_access".to_string()));
	assert_eq!(token.refresh_token, Some("new_refresh".to_string()));
	assert_eq!(token.expires_in, Some(3600));

	// Cleanup
	clear_token();
}

#[aidoku_test]
fn set_token_handles_empty_string() {
	// Test: Store empty string
	set_token(String::new());

	// Should be able to call without panic
	let result = get_token();
	// May succeed or fail depending on JSON parsing, but should not panic
	let _ = result;

	// Cleanup
	clear_token();
}

#[aidoku_test]
fn token_response_serialization() {
	// Test: Complete token
	let complete_token = TokenResponse {
		access_token: Some("access123".to_string()),
		refresh_token: Some("refresh456".to_string()),
		expires_in: Some(3600),
	};

	let json = to_string(&complete_token).unwrap();
	assert!(json.contains("access123"));
	assert!(json.contains("refresh456"));
	assert!(json.contains("3600"));
}

#[aidoku_test]
fn token_response_partial_serialization() {
	// Test: Token with only access token
	let partial_token = TokenResponse {
		access_token: Some("only_access".to_string()),
		refresh_token: None,
		expires_in: None,
	};

	let json = to_string(&partial_token).unwrap();
	assert!(json.contains("only_access"));
	// Should handle None values gracefully
	let _ = json;
}

#[aidoku_test]
fn token_response_deserialization() {
	let json = r#"{"access_token":"test_access","refresh_token":"test_refresh","expires_in":7200}"#;

	let result: Result<TokenResponse, _> = from_str(json);
	assert!(result.is_ok());

	let token = result.unwrap();
	assert_eq!(token.access_token, Some("test_access".to_string()));
	assert_eq!(token.refresh_token, Some("test_refresh".to_string()));
	assert_eq!(token.expires_in, Some(7200));
}

#[aidoku_test]
fn token_response_deserialization_partial() {
	let json = r#"{"access_token":"partial_access"}"#;

	let result: Result<TokenResponse, _> = from_str(json);
	assert!(result.is_ok());

	let token = result.unwrap();
	assert_eq!(token.access_token, Some("partial_access".to_string()));
	assert_eq!(token.refresh_token, None);
	assert_eq!(token.expires_in, None);
}

#[aidoku_test]
fn token_response_deserialization_null_values() {
	let json = r#"{"access_token":null,"refresh_token":null,"expires_in":null}"#;

	let result: Result<TokenResponse, _> = from_str(json);
	assert!(result.is_ok());

	let token = result.unwrap();
	assert_eq!(token.access_token, None);
	assert_eq!(token.refresh_token, None);
	assert_eq!(token.expires_in, None);
}

#[aidoku_test]
fn refresh_token_no_current_token() {
	// Ensure no token is stored
	clear_token();

	// Test: Should fail when no token exists
	let result = refresh_token();
	assert!(result.is_err());
}

#[aidoku_test]
fn refresh_token_no_refresh_token() {
	// Setup: Token with no refresh token
	let token_json = create_test_token(Some("access_only"), None, Some(3600));
	set_token(token_json);

	// Test: Should fail when no refresh token exists
	let result = refresh_token();
	assert!(result.is_err());

	// Cleanup
	clear_token();
}

#[aidoku_test]
fn auth_request_format_validation() {
	// Test: Auth header format should be correct
	let access_token = "test_token_123";
	let expected_header = format!("{} {}", AUTH_SCHEME, access_token);

	assert!(expected_header.starts_with("Bearer "));
	assert!(expected_header.contains(access_token));
	assert_eq!(expected_header.split_whitespace().count(), 2);
}

#[aidoku_test]
fn token_storage_stress_test() {
	// Stress test token storage with rapid operations
	for i in 0..50 {
		let token_json = create_test_token(
			Some(&format!("access_{}", i)),
			Some(&format!("refresh_{}", i)),
			Some(3600 + i),
		);

		set_token(token_json);

		// Should be able to retrieve immediately
		let result = get_token();
		assert!(result.is_ok());

		let token = result.unwrap();
		assert_eq!(token.access_token, Some(format!("access_{}", i)));
	}

	// Cleanup
	clear_token();
}

#[aidoku_test]
fn token_persistence() {
	// Test: Token should persist across multiple get operations
	let token_json = create_test_token(
		Some("persistent_token"),
		Some("persistent_refresh"),
		Some(1800),
	);
	set_token(token_json);

	// Multiple reads should return same data
	for _ in 0..10 {
		let result = get_token();
		assert!(result.is_ok());

		let token = result.unwrap();
		assert_eq!(token.access_token, Some("persistent_token".to_string()));
		assert_eq!(token.refresh_token, Some("persistent_refresh".to_string()));
	}

	// Cleanup
	clear_token();
}

#[aidoku_test]
fn token_boundary_values() {
	// Test with boundary values for expires_in
	let boundary_values = [0i64, 1, i64::MAX, i64::MIN, -1, 3600, 86400];

	for &expires in &boundary_values {
		let token_json = create_test_token(
			Some("boundary_access"),
			Some("boundary_refresh"),
			Some(expires),
		);
		set_token(token_json);

		let result = get_token();
		assert!(result.is_ok());

		let token = result.unwrap();
		assert_eq!(token.expires_in, Some(expires));
	}

	// Cleanup
	clear_token();
}

#[aidoku_test]
fn token_unicode_handling() {
	// Test with unicode characters in tokens
	let unicode_tokens = [
		"токен_доступа_🔑",
		"アクセストークン",
		"令牌_access",
		"🚀_refresh_token_✨",
	];

	for token_text in &unicode_tokens {
		let token_json = create_test_token(
			Some(token_text),
			Some(&format!("refresh_{}", token_text)),
			Some(3600),
		);
		set_token(token_json);

		let result = get_token();
		assert!(result.is_ok());

		let token = result.unwrap();
		assert_eq!(token.access_token, Some(token_text.to_string()));
	}

	// Cleanup
	clear_token();
}
