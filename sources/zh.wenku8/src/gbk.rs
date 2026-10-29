//! GBK (CP936) codec backed by `encoding_rs` (the WHATWG Encoding Standard
//! implementation, same library the reference client uses).
//!
//! aidoku-rs responses are UTF-8 only, while wenku8 serves every desktop
//! page in GBK, so decoding happens here on the raw bytes. The convenient
//! `Encoding::decode`/`encode` methods are std-only, so this crate uses the
//! raw no-alloc encoder API: unmappable characters get a `?` fallback
//! instead of encoding_rs's HTML numeric character references, which a form
//! submission must not contain.

use aidoku::alloc::{string::String, vec::Vec};
use encoding_rs::{EncoderResult, GBK};

/// Decode GBK bytes to a String. Invalid sequences become U+FFFD.
pub fn decode(bytes: &[u8]) -> String {
	let (cow, _) = GBK.decode_without_bom_handling(bytes);
	cow.into_owned()
}

/// Encode a &str as GBK bytes. Unmappable characters become '?'.
pub fn encode(text: &str) -> Vec<u8> {
	let mut encoder = GBK.new_encoder();
	let mut out = Vec::with_capacity(text.len());
	let mut char_buf = [0u8; 4];
	let mut gbk_buf = [0u8; 8]; // GBK uses at most 2 bytes per character
	for ch in text.chars() {
		let (result, _, written) = encoder.encode_from_utf8_without_replacement(
			ch.encode_utf8(&mut char_buf),
			&mut gbk_buf,
			true,
		);
		match result {
			EncoderResult::Unmappable(_) => out.push(b'?'),
			_ => out.extend_from_slice(&gbk_buf[..written]),
		}
	}
	out
}

/// Percent-encode text as GBK bytes with everything except alphanumerics
/// and `_` escaped (matching the reference client behavior) — for the
/// desktop site, which speaks GBK.
pub fn url_encode(text: &str) -> String {
	const HEX: &[u8; 16] = b"0123456789ABCDEF";
	let bytes = encode(text);
	let mut out = String::with_capacity(bytes.len() * 3);
	for b in bytes {
		if b.is_ascii_alphanumeric() || b == b'_' {
			out.push(b as char);
		} else {
			out.push('%');
			out.push(HEX[(b >> 4) as usize] as char);
			out.push(HEX[(b & 0x0F) as usize] as char);
		}
	}
	out
}

/// Percent-encode UTF-8 text the way the WAP endpoints expect: every byte
/// except alphanumerics and `_` escaped.
pub fn url_encode_utf8(text: &str) -> String {
	const HEX: &[u8; 16] = b"0123456789ABCDEF";
	let mut out = String::with_capacity(text.len() * 3);
	for &b in text.as_bytes() {
		if b.is_ascii_alphanumeric() || b == b'_' {
			out.push(b as char);
		} else {
			out.push('%');
			out.push(HEX[(b >> 4) as usize] as char);
			out.push(HEX[(b & 0x0F) as usize] as char);
		}
	}
	out
}
