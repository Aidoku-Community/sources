use aidoku::{
    alloc::{String, string::ToString},
    imports::defaults::{DefaultValue, defaults_get, defaults_set},
};

// === Constants ===
const TOKEN_KEY: &str = "auth_token";
const AUTO_CHECKIN_KEY: &str = "autoCheckin";
const LAST_CHECKIN_KEY: &str = "lastCheckin";
const ENHANCED_MODE_KEY: &str = "enhancedMode";
const JUST_LOGGED_IN_KEY: &str = "justLoggedIn";

// === Auth Token ===

/// Retrieve the stored auth token, if valid.
pub fn get_token() -> Option<String> {
    defaults_get::<String>(TOKEN_KEY).filter(|s| !s.is_empty())
}

pub fn set_token(token: &str) {
    defaults_set(TOKEN_KEY, DefaultValue::String(token.to_string()));
}

pub fn clear_token() {
    defaults_set(TOKEN_KEY, DefaultValue::Null);
}

pub fn get_auto_checkin() -> bool {
    defaults_get::<bool>(AUTO_CHECKIN_KEY).unwrap_or(false)
}

pub fn has_checkin_flag() -> bool {
    defaults_get::<String>(LAST_CHECKIN_KEY)
        .filter(|s| !s.is_empty())
        .is_some()
}

pub fn set_last_checkin(date: &str) {
    defaults_set(LAST_CHECKIN_KEY, DefaultValue::String(date.into()));
}

pub fn clear_checkin_flag() {
    defaults_set(LAST_CHECKIN_KEY, DefaultValue::Null);
}

// === Enhanced Mode ===

/// Returns true if Enhanced Mode is enabled AND the user is logged in.
pub fn get_enhanced_mode() -> bool {
    // Only return true if setting is enabled AND user is logged in
    defaults_get::<bool>(ENHANCED_MODE_KEY).unwrap_or(false) && get_token().is_some()
}


// === Login State Flag ===

pub fn set_just_logged_in() {
    defaults_set(JUST_LOGGED_IN_KEY, DefaultValue::Bool(true));
}

pub fn is_just_logged_in() -> bool {
    defaults_get::<bool>(JUST_LOGGED_IN_KEY).unwrap_or(false)
}

pub fn clear_just_logged_in() {
    defaults_set(JUST_LOGGED_IN_KEY, DefaultValue::Null);
}
