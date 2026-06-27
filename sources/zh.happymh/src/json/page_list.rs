use crate::BASE_URL;
use aidoku::{
	Page, Result,
	alloc::{String, Vec, string::ToString as _, vec},
	error,
	imports::{net::Request, std::current_date},
	prelude::*,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha2::{Digest as _, Sha256};

pub struct PageList;

impl PageList {
	pub fn get_pages(manga_id: String, chapter_id: String) -> Result<Vec<Page>> {
		let request_id = (current_date() * 1000).to_string();
		let ga_timestamp = generate_ga_timestamp();
		let url = format!(
			"{}/v2.0/apis/manga/reading?code={}&cid={}&v=v4.300101&_t={}",
			BASE_URL, manga_id, chapter_id, request_id
		);
		let json: serde_json::Value = Request::get(url.clone())?
			.header(
				"Referer",
				&format!("{}/mangaread/{}/{}", BASE_URL, manga_id, chapter_id),
			)
			.header("Origin", BASE_URL)
			.header("X-Requested-With", "XMLHttpRequest")
			.header("X-Requested-Id", &request_id)
			.header("Accept", "application/json")
			.header(
				"Cookie",
				&format!(
					"_ga_HVJMXGJXFJ=GS2.1.s{}$o9$g1$t{}$j43$l0$h0",
					ga_timestamp,
					ga_timestamp + 99999
				),
			)
			.send()?
			.get_json()?;
		let data = json
			.as_object()
			.ok_or_else(|| error!("Expected JSON object"))?;
		let data = data
			.get("data")
			.and_then(|v| v.as_object())
			.ok_or_else(|| error!("Expected data object"))?;
		let is_encode = data
			.get("isEncode")
			.and_then(|v| v.as_bool())
			.unwrap_or(false);
		let scans = data
			.get("scans")
			.ok_or_else(|| error!("Expected scans field"))?;
		let list: Vec<serde_json::Value> = if let Some(arr) = scans.as_array() {
			arr.clone()
		} else if let Some(s) = scans.as_str() {
			let scans = if is_encode {
				decode_scans(s)?
			} else {
				s.to_string()
			};
			let parsed: serde_json::Value = serde_json::from_str(&scans)
				.map_err(|_| error!("Failed to parse scans JSON string"))?;
			parsed
				.as_array()
				.ok_or_else(|| error!("Expected scans array after parsing"))?
				.clone()
		} else {
			bail!("Expected scans array or JSON string");
		};
		let mut pages: Vec<Page> = Vec::new();

		for item in list.iter() {
			let item = match item.as_object() {
				Some(item) => item,
				None => continue,
			};

			// Skip images from next chapter (n == 1)
			let n = item.get("n").and_then(|v| v.as_i64()).unwrap_or(0);
			if n != 0 {
				continue;
			}

			let mut url = item
				.get("url")
				.and_then(|v| v.as_str())
				.unwrap_or_default()
				.to_string();

			if let Some(stripped) = url.split("?q=").next() {
				url = stripped.to_string();
			}

			pages.push(Page {
				content: aidoku::PageContent::url(url),
				..Default::default()
			});
		}

		Ok(pages)
	}
}

fn generate_ga_timestamp() -> i64 {
	const TABLE: [i64; 10] = [335, 984, 248, 485, 524, 559, 486, 165, 114, 103];
	let seconds = current_date();
	let digits = seconds.to_string();
	let bytes = digits.as_bytes();
	let len = bytes.len();
	let sum = TABLE[(bytes[len - 3] - b'0') as usize]
		+ TABLE[(bytes[len - 2] - b'0') as usize]
		+ TABLE[(bytes[len - 1] - b'0') as usize];
	let checksum = sum.to_string();
	let checksum = &checksum[..3];
	format!("{}{}", digits, checksum)
		.parse()
		.unwrap_or(seconds * 1000)
}

fn decode_scans(encrypted_scans: &str) -> Result<String> {
	const SECRET: &[u8] = b"DEV_SCAN_SECRET_2026_change_me";
	const DOMAIN: &[u8] = b"happymh.com";

	let buf = encrypted_scans.as_bytes();
	if buf.len() < 8 {
		bail!("Invalid encoded scans");
	}

	let digest = sha256(&[SECRET, &buf[..8], DOMAIN].concat());
	let off1 = (digest[0] as usize) % 24 + 8;
	let off2 = (digest[1] as usize) % 24 + 8;
	let off3 = (digest[2] as usize) % 24 + 8;

	let key_start = off1 + 8;
	let key_end = key_start + 64;
	let nonce_start = key_end + off2;
	let nonce_end = nonce_start + 32;
	let cipher_start = nonce_end + off3;
	if encrypted_scans.len() < cipher_start {
		bail!("Encoded scans too short for computed offsets");
	}

	let key = decode_hex(&encrypted_scans[key_start..key_end])?;
	let nonce = decode_hex(&encrypted_scans[nonce_start..nonce_end])?;
	let ciphertext = STANDARD
		.decode(&encrypted_scans[cipher_start..])
		.map_err(|_| error!("Failed to decode scans base64"))?;

	let mut state = [0_u8; 52];
	state[..32].copy_from_slice(&key);
	state[32..48].copy_from_slice(&nonce);

	let mut plain = vec![0_u8; ciphertext.len()];
	for i in (0..ciphertext.len()).step_by(32) {
		let block_idx = (i / 32) as u32;
		state[48..52].copy_from_slice(&block_idx.to_be_bytes());
		let keystream = sha256(&state);
		let block_size = core::cmp::min(32, ciphertext.len() - i);
		for j in 0..block_size {
			plain[i + j] = ciphertext[i + j] ^ keystream[j];
		}
	}

	if !plain.starts_with(b"SC01") {
		bail!("Decoding scans failed");
	}

	let decompressed = miniz_oxide::inflate::decompress_to_vec(&plain[4..])
		.map_err(|_| error!("Failed to decompress scans"))?;
	String::from_utf8(decompressed).map_err(|_| error!("Failed to decode scans utf8"))
}

fn sha256(data: &[u8]) -> [u8; 32] {
	let mut hasher = Sha256::new();
	hasher.update(data);
	hasher.finalize().into()
}

fn decode_hex(input: &str) -> Result<Vec<u8>> {
	if !input.len().is_multiple_of(2) {
		bail!("Invalid hex string");
	}
	let mut out = Vec::with_capacity(input.len() / 2);
	for chunk in input.as_bytes().chunks(2) {
		let high = hex_value(chunk[0])?;
		let low = hex_value(chunk[1])?;
		out.push((high << 4) | low);
	}
	Ok(out)
}

fn hex_value(byte: u8) -> Result<u8> {
	match byte {
		b'0'..=b'9' => Ok(byte - b'0'),
		b'a'..=b'f' => Ok(byte - b'a' + 10),
		b'A'..=b'F' => Ok(byte - b'A' + 10),
		_ => bail!("Invalid hex character"),
	}
}
