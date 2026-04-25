#![allow(dead_code)]

use aidoku::{
	Result,
	alloc::{String, Vec},
	imports::error::AidokuError,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use core::sync::atomic::{AtomicU64, Ordering};
use rsa::pss::SigningKey;
use rsa::RsaPrivateKey;
use rsa::pkcs8::DecodePrivateKey;
use rsa::rand_core::{CryptoRng, RngCore};
use rsa::signature::{RandomizedSigner, SignatureEncoding};
use sha1::Sha1;
use sha2::{Digest as Sha2Digest, Sha256};

static GETRANDOM_STATE: AtomicU64 = AtomicU64::new(0x853c49e6748fea9b);

fn custom_getrandom(buf: &mut [u8]) -> core::result::Result<(), getrandom::Error> {
	for byte in buf.iter_mut() {
		let mut x = GETRANDOM_STATE.load(Ordering::Relaxed);
		x ^= x << 13;
		x ^= x >> 7;
		x ^= x << 17;
		GETRANDOM_STATE.store(x, Ordering::Relaxed);
		*byte = x as u8;
	}
	Ok(())
}
getrandom::register_custom_getrandom!(custom_getrandom);

pub struct XorShift64(u64);

impl XorShift64 {
	pub const fn new() -> Self {
		Self(0x853c49e6748fea9b)
	}
}

impl RngCore for XorShift64 {
	fn next_u32(&mut self) -> u32 {
		self.next_u64() as u32
	}

	fn next_u64(&mut self) -> u64 {
		let mut x = self.0;
		x ^= x << 13;
		x ^= x >> 7;
		x ^= x << 17;
		self.0 = x;
		x
	}

	fn fill_bytes(&mut self, dest: &mut [u8]) {
		let mut i = 0;
		while i + 8 <= dest.len() {
			let n = self.next_u64().to_le_bytes();
			dest[i..i + 8].copy_from_slice(&n);
			i += 8;
		}
		if i < dest.len() {
			let n = self.next_u64().to_le_bytes();
			for j in 0..(dest.len() - i) {
				dest[i + j] = n[j];
			}
		}
	}

	fn try_fill_bytes(&mut self, dest: &mut [u8]) -> core::result::Result<(), rsa::rand_core::Error> {
		self.fill_bytes(dest);
		Ok(())
	}
}

impl CryptoRng for XorShift64 {}

pub struct WvdData {
	pub device_type: u8,
	pub private_key_der: Vec<u8>,
	pub client_id: Vec<u8>,
}

pub fn parse_wvd(data: &[u8]) -> Result<WvdData> {
	if data.len() < 9 {
		return Err(AidokuError::message("WVD too short"));
	}
	if &data[0..3] != b"WVD" {
		return Err(AidokuError::message("Invalid WVD magic"));
	}
	if data[3] != 2 {
		return Err(AidokuError::message("Unsupported WVD version (expected 2)"));
	}

	let device_type = data[4];
	let pk_len = u16::from_be_bytes([data[7], data[8]]) as usize;
	let pk_end = 9 + pk_len;
	if data.len() < pk_end + 2 {
		return Err(AidokuError::message("WVD truncated (private key)"));
	}

	let private_key_der = data[9..pk_end].to_vec();
	let ci_len = u16::from_be_bytes([data[pk_end], data[pk_end + 1]]) as usize;
	let ci_start = pk_end + 2;
	if data.len() < ci_start + ci_len {
		return Err(AidokuError::message("WVD truncated (client_id)"));
	}

	let client_id = data[ci_start..ci_start + ci_len].to_vec();
	Ok(WvdData {
		device_type,
		private_key_der,
		client_id,
	})
}

fn proto_write_varint(buf: &mut Vec<u8>, mut v: u64) {
	loop {
		if v < 0x80 {
			buf.push(v as u8);
			return;
		}
		buf.push((v as u8) | 0x80);
		v >>= 7;
	}
}

fn proto_write_bytes_field(buf: &mut Vec<u8>, field: u32, data: &[u8]) {
	proto_write_varint(buf, (field as u64) << 3 | 2);
	proto_write_varint(buf, data.len() as u64);
	buf.extend_from_slice(data);
}

fn proto_write_varint_field(buf: &mut Vec<u8>, field: u32, value: u64) {
	proto_write_varint(buf, (field as u64) << 3);
	proto_write_varint(buf, value);
}

const SYSTEM_ID: [u8; 16] = [
	0xED, 0xEF, 0x8B, 0xA9, 0x79, 0xD6, 0x2A, 0xCE, 0xA3, 0xC8, 0x27, 0xDC, 0xD5,
	0x1D, 0x21, 0xED,
];

pub fn build_pssh(f: &[u8; 16]) -> Vec<u8> {
	let mut widevine_data = Vec::new();
	proto_write_bytes_field(&mut widevine_data, 2, f);

	let total_size = 32 + widevine_data.len();
	let mut pssh = Vec::with_capacity(total_size);
	pssh.extend_from_slice(&(total_size as u32).to_be_bytes());
	pssh.extend_from_slice(b"pssh");
	pssh.extend_from_slice(&[0u8; 4]);
	pssh.extend_from_slice(&SYSTEM_ID);
	pssh.extend_from_slice(&(widevine_data.len() as u32).to_be_bytes());
	pssh.extend_from_slice(&widevine_data);
	pssh
}

pub fn extract_init_data(pssh: &[u8]) -> Result<Vec<u8>> {
	if pssh.len() < 32 {
		return Err(AidokuError::message("PSSH too short"));
	}
	let data_size = u32::from_be_bytes([pssh[28], pssh[29], pssh[30], pssh[31]]) as usize;
	if pssh.len() < 32 + data_size {
		return Err(AidokuError::message("PSSH data truncated"));
	}
	Ok(pssh[32..32 + data_size].to_vec())
}

pub fn encode_license_request(
	client_id: &[u8],
	init_data: &[u8],
	request_id: &[u8],
	request_time: i64,
	nonce: u32,
) -> Vec<u8> {
	let mut pssh_data_msg = Vec::new();
	proto_write_bytes_field(&mut pssh_data_msg, 1, init_data);
	proto_write_varint_field(&mut pssh_data_msg, 2, 1);
	proto_write_bytes_field(&mut pssh_data_msg, 3, request_id);

	let mut content_id = Vec::new();
	proto_write_bytes_field(&mut content_id, 1, &pssh_data_msg);

	let mut msg = Vec::new();
	proto_write_bytes_field(&mut msg, 1, client_id);
	proto_write_bytes_field(&mut msg, 2, &content_id);
	proto_write_varint_field(&mut msg, 3, 1);
	proto_write_varint_field(&mut msg, 4, request_time as u64);
	proto_write_varint_field(&mut msg, 6, 21);
	proto_write_varint_field(&mut msg, 7, nonce as u64);
	msg
}

pub fn encode_signed_message(msg: &[u8], signature: &[u8]) -> Vec<u8> {
	let mut out = Vec::new();
	proto_write_varint_field(&mut out, 1, 1);
	proto_write_bytes_field(&mut out, 2, msg);
	proto_write_bytes_field(&mut out, 3, signature);
	out
}

fn wrap_pkcs1_in_pkcs8(pkcs1: &[u8]) -> Vec<u8> {
	const HEADER: [u8; 26] = [
		0x30, 0x82, 0x00, 0x00, 0x02, 0x01, 0x00, 0x30, 0x0d, 0x06, 0x09, 0x2a, 0x86,
		0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01, 0x05, 0x00, 0x04, 0x82, 0x00, 0x00,
	];
	let total = 26 + pkcs1.len();
	let mut out = Vec::with_capacity(total);
	out.extend_from_slice(&HEADER);
	out.extend_from_slice(pkcs1);

	let outer_len = (total - 4) as u16;
	out[2] = (outer_len >> 8) as u8;
	out[3] = (outer_len & 0xff) as u8;

	let inner_len = pkcs1.len() as u16;
	out[24] = (inner_len >> 8) as u8;
	out[25] = (inner_len & 0xff) as u8;
	out
}

pub fn parse_private_key(der: &[u8]) -> Result<RsaPrivateKey> {
	if let Ok(key) = RsaPrivateKey::from_pkcs8_der(der) {
		return Ok(key);
	}

	let pkcs8 = wrap_pkcs1_in_pkcs8(der);
	RsaPrivateKey::from_pkcs8_der(&pkcs8)
		.map_err(|_| AidokuError::message("Failed to parse RSA private key from WVD"))
}

fn to_hex_upper(bytes: &[u8]) -> String {
	use core::fmt::Write;

	let mut s = String::new();
	for b in bytes {
		let _ = write!(s, "{b:02X}");
	}
	s
}

pub fn generate_challenge(wvd_base64: &str, chapter_id: &str) -> Result<String> {
	if wvd_base64.is_empty() {
		return Err(AidokuError::message(
			"No WVD key configured. Add your WVD file (base64) in source settings.",
		));
	}

	let wvd_bytes = STANDARD
		.decode(wvd_base64)
		.map_err(|_| AidokuError::message("Invalid WVD base64 encoding"))?;
	let wvd = parse_wvd(&wvd_bytes)?;

	let mut hasher = Sha256::new();
	hasher.update(b":");
	hasher.update(chapter_id.as_bytes());
	let hash = hasher.finalize();
	let f: [u8; 16] = hash[..16]
		.try_into()
		.map_err(|_| AidokuError::message("SHA256 slice error"))?;

	let pssh = build_pssh(&f);
	let init_data = extract_init_data(&pssh)?;

	let mut rng = XorShift64::new();
	let request_id: Vec<u8> = if wvd.device_type == 2 {
		let mut random = [0u8; 4];
		rng.fill_bytes(&mut random);
		let mut id_bytes = [0u8; 16];
		id_bytes[..4].copy_from_slice(&random);
		id_bytes[8..].copy_from_slice(&1u64.to_le_bytes());
		to_hex_upper(&id_bytes).into_bytes()
	} else {
		let mut bytes = [0u8; 16];
		rng.fill_bytes(&mut bytes);
		bytes.to_vec()
	};

	let request_time = aidoku::imports::std::current_date();
	let nonce = rng.next_u32();
	let license_request =
		encode_license_request(&wvd.client_id, &init_data, &request_id, request_time, nonce);

	let private_key = parse_private_key(&wvd.private_key_der)?;
	let signing_key = SigningKey::<Sha1>::new(private_key);
	let signature = signing_key.sign_with_rng(&mut rng, &license_request);
	let signed = encode_signed_message(&license_request, &signature.to_vec());
	Ok(STANDARD.encode(signed))
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	fn make_wvd(device_type: u8, private_key: &[u8], client_id: &[u8]) -> Vec<u8> {
		use aidoku::alloc::vec;
		let pk_len = private_key.len() as u16;
		let ci_len = client_id.len() as u16;
		let mut v = vec![
			b'W',
			b'V',
			b'D',
			2,
			device_type,
			3,
			0,
			(pk_len >> 8) as u8,
			(pk_len & 0xff) as u8,
		];
		v.extend_from_slice(private_key);
		v.push((ci_len >> 8) as u8);
		v.push((ci_len & 0xff) as u8);
		v.extend_from_slice(client_id);
		v
	}

	#[aidoku_test]
	fn test_parse_wvd_valid() {
		let pk = b"fake-private-key";
		let ci = b"fake-client-id";
		let wvd_bytes = make_wvd(2, pk, ci);
		let result = parse_wvd(&wvd_bytes).unwrap();
		assert_eq!(result.device_type, 2);
		assert_eq!(result.private_key_der, pk.as_slice());
		assert_eq!(result.client_id, ci.as_slice());
	}

	#[aidoku_test]
	fn test_parse_wvd_bad_magic() {
		let bad = b"BAD\x02\x02\x03\x00\x00\x00\x00\x00";
		assert!(parse_wvd(bad).is_err());
	}

	#[aidoku_test]
	fn test_parse_wvd_bad_version() {
		let pk = b"key";
		let mut wvd = make_wvd(1, pk, b"cid");
		wvd[3] = 1;
		assert!(parse_wvd(&wvd).is_err());
	}

	#[aidoku_test]
	fn test_build_pssh_length() {
		let f = [0u8; 16];
		let pssh = build_pssh(&f);
		assert_eq!(pssh.len(), 32 + 18);
	}

	#[aidoku_test]
	fn test_build_pssh_magic() {
		let f = [0u8; 16];
		let pssh = build_pssh(&f);
		assert_eq!(&pssh[4..8], b"pssh");
	}

	#[aidoku_test]
	fn test_extract_init_data_roundtrip() {
		let f = [0xABu8; 16];
		let pssh = build_pssh(&f);
		let init_data = extract_init_data(&pssh).unwrap();
		assert_eq!(init_data[0], 0x12);
		assert_eq!(init_data[1], 0x10);
		assert_eq!(&init_data[2..18], &f);
	}

	#[aidoku_test]
	fn test_proto_varint_single_byte() {
		let mut buf = Vec::new();
		proto_write_varint(&mut buf, 127);
		assert_eq!(buf, &[0x7f]);
	}

	#[aidoku_test]
	fn test_proto_varint_multi_byte() {
		let mut buf = Vec::new();
		proto_write_varint(&mut buf, 300);
		assert_eq!(buf, &[0xAC, 0x02]);
	}

	#[aidoku_test]
	fn test_encode_signed_message_structure() {
		let msg = b"hello";
		let sig = b"sig-bytes";
		let out = encode_signed_message(msg, sig);
		assert_eq!(out[0], 0x08);
		assert_eq!(out[1], 0x01);
		assert_eq!(out[2], 0x12);
		assert_eq!(out[3], 0x05);
		assert_eq!(&out[4..9], b"hello");
	}

	#[aidoku_test]
	fn test_encode_license_request_non_empty() {
		let client_id = b"cid";
		let init_data = b"init";
		let request_id = b"rid";
		let out = encode_license_request(client_id, init_data, request_id, 1700000000, 42);
		assert!(!out.is_empty());
		assert_eq!(out[0], 0x0A);
	}

	#[aidoku_test]
	fn test_parse_rsa_key_invalid_bytes() {
		let result = parse_private_key(b"not-a-real-key-just-garbage-bytes-here");
		assert!(result.is_err());
	}

	#[aidoku_test]
	fn test_generate_challenge_requires_valid_wvd() {
		let result = generate_challenge("", "chapter-id-123");
		assert!(result.is_err());
	}

	#[aidoku_test]
	fn test_generate_challenge_invalid_base64() {
		let result = generate_challenge("not!valid!base64!!!", "chapter-id-123");
		assert!(result.is_err());
	}
}
