use aidoku::{
	AidokuError, Result,
	alloc::{String, format},
	imports::{net::Request, std::parse_date},
};
use hmac_sha256::HMAC;

use crate::BASE_URL;
use crate::models::InertiaPage;

/// Extract and HTML-decode the Inertia `data-page` JSON, then deserialize it.
pub fn parse_inertia<T: serde::de::DeserializeOwned>(html: &str) -> Option<T> {
	let marker = "data-page=\"";
	let start = html.find(marker)? + marker.len();
	let rest = &html[start..];
	let end = rest.find("\">")?;
	let encoded = &rest[..end];

	let decoded = encoded
		.replace("&quot;", "\"")
		.replace("&amp;", "&")
		.replace("&#039;", "'")
		.replace("&lt;", "<")
		.replace("&gt;", ">");

	serde_json::from_str::<InertiaPage<T>>(&decoded)
		.ok()
		.map(|p| p.props)
}

/// Decode a hex string into bytes.
pub fn decode_hex(s: &str) -> Option<[u8; 32]> {
	if s.len() != 64 {
		return None;
	}
	let mut out = [0u8; 32];
	let bytes = s.as_bytes();
	for (i, chunk) in bytes.chunks(2).enumerate() {
		let hi = hex_digit(chunk[0])?;
		let lo = hex_digit(chunk[1])?;
		out[i] = (hi << 4) | lo;
	}
	Some(out)
}

fn hex_digit(b: u8) -> Option<u8> {
	match b {
		b'0'..=b'9' => Some(b - b'0'),
		b'a'..=b'f' => Some(b - b'a' + 10),
		b'A'..=b'F' => Some(b - b'A' + 10),
		_ => None,
	}
}

/// Encode bytes as a lowercase hex string.
pub fn to_hex(bytes: &[u8]) -> String {
	const HEX: &[u8; 16] = b"0123456789abcdef";
	let mut out = String::with_capacity(bytes.len() * 2);
	for &b in bytes {
		out.push(HEX[(b >> 4) as usize] as char);
		out.push(HEX[(b & 0xf) as usize] as char);
	}
	out
}

/// Build a signed page image URL using pure-Rust HMAC-SHA256.
///
/// Signing scheme (observed from pam.wasm):
///   message = hex(page as u8) + decimal_ts_string + hex_nonce (16 hex chars)
///   key     = chapter_token hex-decoded (32 bytes)
///   sig     = HMAC-SHA256(key, message) as lowercase hex
pub fn build_page_url(
	serie_slug: &str,
	chapter_slug: &str,
	token: &str,
	page: i32,
	ts: i64,
	nonce: &[u8; 8],
) -> String {
	let key = match decode_hex(token) {
		Some(k) => k,
		None => {
			return format!(
				"{BASE_URL}/serie/{serie_slug}/chapter/{chapter_slug}/page/{page}?token={token}"
			);
		}
	};

	let nonce_hex = to_hex(nonce);
	let msg = format!("{:02x}{ts}{nonce_hex}", page as u8);

	let sig_bytes = HMAC::mac(msg.as_bytes(), key);
	let sig = to_hex(&sig_bytes);

	format!(
		"{BASE_URL}/serie/{serie_slug}/chapter/{chapter_slug}/page/{page}?token={token}&ts={ts}&nonce={nonce_hex}&sig={sig}"
	)
}

/// Parse an ISO-8601 datetime string to a Unix timestamp using aidoku's parse_date.
pub fn parse_chapter_date(s: &str) -> Option<i64> {
	// Input format: "2026-07-17T01:24:53.000000Z"
	// Trim to "2026-07-17T01:24:53" for the format string
	let trimmed = if s.len() >= 19 { &s[..19] } else { s };
	parse_date(trimmed, "yyyy-MM-dd'T'HH:mm:ss")
}

/// Build an absolute URL from a potentially relative path.
pub fn abs_url(path: &str) -> String {
	if path.starts_with('/') {
		format!("{BASE_URL}{path}")
	} else {
		String::from(path)
	}
}

/// Fetch a URL with a mobile User-Agent to avoid Cloudflare blocks.
pub fn fetch_html(url: &str) -> Result<String> {
	Request::get(url)
		.map_err(|e| AidokuError::Message(format!("request error: {:?}", e)))?
		.header(
			"User-Agent",
			"Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) \
			 AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1",
		)
		.header("Referer", BASE_URL)
		.string()
}
