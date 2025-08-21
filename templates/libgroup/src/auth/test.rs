use crate::auth::{get_token, get_user_id, refresh_token, set_token, set_token_and_user_id};
use aidoku::{
	alloc::{String, string::ToString},
	imports::defaults::{DefaultValue, defaults_set},
	prelude::*,
};
use aidoku_test::aidoku_test;
use serde_json::{from_str, to_string};

use crate::{
	auth::{AUTH_SCHEME, TOKEN_KEY, USER_ID_KEY},
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

// Test helper to clear stored token and user ID
fn clear_auth_data() {
	defaults_set(TOKEN_KEY, DefaultValue::String(String::new()));
	defaults_set(USER_ID_KEY, DefaultValue::Int(0));
}

// Test helper to set user ID directly
fn set_test_user_id(user_id: i32) {
	defaults_set(USER_ID_KEY, DefaultValue::Int(user_id));
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
	clear_auth_data();
}

#[aidoku_test]
fn get_token_no_token_stored() {
	// Ensure no token is stored
	clear_auth_data();

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
	clear_auth_data();
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
	clear_auth_data();
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
	clear_auth_data();
}

#[aidoku_test]
fn get_user_id_existing_stored() {
	// Setup: Store a user ID
	set_test_user_id(12345);

	// Test: Should return stored user ID
	let result = get_user_id();
	assert_eq!(result, Some(12345));

	// Cleanup
	clear_auth_data();
}

#[aidoku_test]
fn get_user_id_zero_value() {
	// Setup: Store zero (invalid) user ID
	set_test_user_id(0);

	// Test: Should return None for zero value
	let result = get_user_id();
	assert_eq!(result, None);

	// Cleanup
	clear_auth_data();
}

#[aidoku_test]
fn get_user_id_no_stored_no_token() {
	// Ensure no auth data is stored
	clear_auth_data();

	// Test: Should return None when no user ID and no token
	let result = get_user_id();
	assert_eq!(result, None);
}

#[aidoku_test]
fn set_token_and_user_id_stores_token() {
	let token_json = create_test_token(Some("combo_access"), Some("combo_refresh"), Some(3600));

	// Test: Should store token (user ID fetch will fail in test env but token should be stored)
	let _ = set_token_and_user_id(token_json);

	// Verify: Token should be retrievable
	let result = get_token();
	assert!(result.is_ok());

	let token = result.unwrap();
	assert_eq!(token.access_token, Some("combo_access".to_string()));

	// Cleanup
	clear_auth_data();
}

#[aidoku_test]
fn refresh_token_no_current_token() {
	// Ensure no token is stored
	clear_auth_data();

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
	clear_auth_data();
}

#[aidoku_test]
fn token_response_serialization() {
	// Test: Complete token serialization
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
fn auth_request_format_validation() {
	// Test: Auth header format should be correct
	let access_token = "test_token_123";
	let expected_header = format!("{} {}", AUTH_SCHEME, access_token);

	assert!(expected_header.starts_with("Bearer "));
	assert!(expected_header.contains(access_token));
	assert_eq!(expected_header.split_whitespace().count(), 2);
}
