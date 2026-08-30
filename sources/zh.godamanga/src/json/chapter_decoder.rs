use aidoku::alloc::string::ToString as _;
use aidoku::{
	Result,
	alloc::{String, Vec},
	error,
};

const STD_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
const CUSTOM_ALPHABET: &[u8] = b"_-9876543210abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

pub struct ChapterImageDecoder;

impl ChapterImageDecoder {
	const PREFIX: &'static str = "J7r";
	const MARKER1: &'static str = "kD";
	const MARKER2: &'static str = "W4s";
	const SUFFIX: &'static str = "nQ";
	const GROUP: usize = 7;

	pub fn decode(input: &str) -> Result<String> {
		if !input.starts_with(Self::PREFIX) || !input.ends_with(Self::SUFFIX) {
			return Err(error!("Unknown chapter data format"));
		}
		let body = &input[Self::PREFIX.len()..input.len() - Self::SUFFIX.len()];
		let payload_len = body.len() - Self::MARKER1.len() - Self::MARKER2.len();
		if payload_len == 0 {
			return Err(error!("Unknown chapter data format"));
		}

		let a_len = payload_len / 3;
		let b_len = (payload_len - a_len) / 2;
		let c_len = payload_len - a_len - b_len;

		let part1 = &body[..b_len];
		let marker1 = &body[b_len..b_len + Self::MARKER1.len()];
		let part2 = &body[b_len + Self::MARKER1.len()..b_len + Self::MARKER1.len() + c_len];
		let marker2 = &body[b_len + Self::MARKER1.len() + c_len
			..b_len + Self::MARKER1.len() + c_len + Self::MARKER2.len()];
		let part3 = &body[b_len + Self::MARKER1.len() + c_len + Self::MARKER2.len()..];

		if marker1 != Self::MARKER1 || marker2 != Self::MARKER2 || part3.len() != a_len {
			return Err(error!("Unknown chapter data format"));
		}

		let reordered = part3.to_string() + part1 + part2;
		let standard = Self::unzigzag(&reordered);
		let mapped = Self::map_alphabet(&standard)?;
		let decoded = Self::base64_url_decode(&mapped)?;
		String::from_utf8(decoded).map_err(|_| error!("Invalid UTF-8 in decoded data"))
	}

	fn unzigzag(s: &str) -> String {
		let mut result = String::with_capacity(s.len());
		let bytes = s.as_bytes();
		let mut i = 0;
		let mut block = 0;
		while i < bytes.len() {
			let end = core::cmp::min(i + Self::GROUP, bytes.len());
			let chunk = &bytes[i..end];
			if block % 2 == 1 {
				for &b in chunk.iter().rev() {
					result.push(b as char);
				}
			} else {
				for &b in chunk.iter() {
					result.push(b as char);
				}
			}
			i += Self::GROUP;
			block += 1;
		}
		result
	}

	fn map_alphabet(s: &str) -> Result<String> {
		let mut result = String::with_capacity(s.len());
		for ch in s.chars() {
			let mapped =
				Self::lookup(ch).ok_or_else(|| error!("Invalid chapter data character"))?;
			result.push(mapped);
		}
		Ok(result)
	}

	fn lookup(ch: char) -> Option<char> {
		for i in 0..CUSTOM_ALPHABET.len() {
			if CUSTOM_ALPHABET[i] == ch as u8 {
				return Some(STD_ALPHABET[i] as char);
			}
		}
		None
	}

	fn base64_url_decode(s: &str) -> Result<Vec<u8>> {
		let mut padded = s.as_bytes().to_vec();
		let pad = (4 - padded.len() % 4) % 4;
		padded.extend(core::iter::repeat_n(b'=', pad));

		let mut output = Vec::with_capacity(padded.len() / 4 * 3);
		let mut buffer = 0u32;
		let mut bits = 0u32;

		for &byte in padded.iter() {
			let value = match byte {
				b'A'..=b'Z' => u32::from(byte - b'A'),
				b'a'..=b'z' => u32::from(byte - b'a' + 26),
				b'0'..=b'9' => u32::from(byte - b'0' + 52),
				b'-' | b'_' => 62,
				b'=' => break,
				_ => return Err(error!("Invalid base64 character")),
			};
			buffer = (buffer << 6) | value;
			bits += 6;
			if bits >= 8 {
				bits -= 8;
				output.push((buffer >> bits) as u8);
			}
		}

		Ok(output)
	}
}
