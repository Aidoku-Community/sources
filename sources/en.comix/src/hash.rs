// reference: https://github.com/keiyoushi/extensions-source/blob/bbffe0d72a7d55ae46d15cecfa1fc77913d8be18/src/en/comix/src/eu/kanade/tachiyomi/extension/en/comix/ComixHash.kt

use aidoku::{
	alloc::{string::String, vec::Vec},
	helpers::uri::encode_uri_component,
	prelude::*,
};
use base64::{
	Engine,
	engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};

// [RC4 key, mutKey, prefKey] × 5 rounds
const KEYS: [&str; 15] = [
	"13YDu67uDgFczo3DnuTIURqas4lfMEPADY6Jaeqky+w=", // 0  RC4 key  round 1
	"yEy7wBfBc+gsYPiQL/4Dfd0pIBZFzMwrtlRQGwMXy3Q=", // 1  mutKey   round 1
	"yrP+EVA1Dw==",                                 // 2  prefKey  round 1
	"vZ23RT7pbSlxwiygkHd1dhToIku8SNHPC6V36L4cnwM=", // 3  RC4 key  round 2
	"QX0sLahOByWLcWGnv6l98vQudWqdRI3DOXBdit9bxCE=", // 4  mutKey   round 2
	"WJwgqCmf",                                     // 5  prefKey  round 2
	"BkWI8feqSlDZKMq6awfzWlUypl88nz65KVRmpH0RWIc=", // 6  RC4 key  round 3
	"v7EIpiQQjd2BGuJzMbBA0qPWDSS+wTJRQ7uGzZ6rJKs=", // 7  mutKey   round 3
	"1SUReYlCRA==",                                 // 8  prefKey  round 3
	"RougjiFHkSKs20DZ6BWXiWwQUGZXtseZIyQWKz5eG34=", // 9  RC4 key  round 4
	"LL97cwoDoG5cw8QmhI+KSWzfW+8VehIh+inTxnVJ2ps=", // 10 mutKey   round 4
	"52iDqjzlqe8=",                                 // 11 prefKey  round 4
	"U9LRYFL2zXU4TtALIYDj+lCATRk/EJtH7/y7qYYNlh8=", // 12 RC4 key  round 5
	"e/GtffFDTvnw7LBRixAD+iGixjqTq9kIZ1m0Hj+s6fY=", // 13 mutKey   round 5
	"xb2XwHNB",                                     // 14 prefKey  round 5
];

fn get_key_bytes(index: usize) -> Vec<u8> {
	let Some(b64) = KEYS.get(index) else {
		return Vec::new();
	};
	STANDARD.decode(b64.as_bytes()).unwrap_or_default()
}

fn rc4(key: &[u8], data: &[u8]) -> Vec<u8> {
	if key.is_empty() {
		return data.to_vec();
	}

	let mut s = [0u8; 256];
	for (i, v) in s.iter_mut().enumerate() {
		*v = i as u8;
	}

	let mut j: usize = 0;
	for i in 0..256usize {
		j = (j + s[i] as usize + key[i % key.len()] as usize) % 256;
		s.swap(i, j);
	}

	let mut i: usize = 0;
	j = 0;
	let mut out = Vec::with_capacity(data.len());

	for &byte in data {
		i = (i + 1) % 256;
		j = (j + s[i] as usize) % 256;
		s.swap(i, j);
		let k = s[(s[i] as usize + s[j] as usize) % 256];
		out.push(byte ^ k);
	}

	out
}

#[inline]
fn mut_s(e: u8) -> u8 {
	((e as u16 + 143) % 256) as u8
}
#[inline]
fn mut_l(e: u8) -> u8 {
	e.rotate_right(1)
}
#[inline]
fn mut_c(e: u8) -> u8 {
	((e as u16 + 115) % 256) as u8
}
#[inline]
fn mut_m(e: u8) -> u8 {
	e ^ 177
}
#[inline]
fn mut_f(e: u8) -> u8 {
	((e as i16 - 188).rem_euclid(256)) as u8
}
#[inline]
fn mut_g(e: u8) -> u8 {
	e.rotate_left(2)
}
#[inline]
fn mut_h(e: u8) -> u8 {
	((e as i16 - 42).rem_euclid(256)) as u8
}
#[inline]
fn mut_dollar(e: u8) -> u8 {
	e.rotate_left(4)
}
#[inline]
fn mut_b(e: u8) -> u8 {
	((e as i16 - 12).rem_euclid(256)) as u8
}
#[inline]
fn mut_underscore(e: u8) -> u8 {
	((e as i16 - 20).rem_euclid(256)) as u8
}
#[inline]
fn mut_y(e: u8) -> u8 {
	e.rotate_right(1)
}
#[inline]
fn mut_k(e: u8) -> u8 {
	((e as i16 - 241).rem_euclid(256)) as u8
}
#[inline]
fn get_mut_key(mk: &[u8], idx: usize) -> u8 {
	let m = idx % 32;
	if !mk.is_empty() && m < mk.len() {
		mk[m]
	} else {
		0
	}
}

fn round1(data: &[u8]) -> Vec<u8> {
	let enc = rc4(&get_key_bytes(0), data);
	let mut_key = get_key_bytes(1);
	let pref_key = get_key_bytes(2);

	let mut out = Vec::with_capacity(enc.len() * 2);
	for (i, &b) in enc.iter().enumerate() {
		if i < 7 && i < pref_key.len() {
			out.push(pref_key[i]);
		}
		let mut v = b ^ get_mut_key(&mut_key, i);
		v = match i % 10 {
			0 | 9 => mut_c(v),
			1 => mut_b(v),
			2 => mut_y(v),
			3 => mut_dollar(v),
			4 | 6 => mut_h(v),
			5 => mut_s(v),
			7 => mut_k(v),
			8 => mut_l(v),
			_ => v,
		};
		out.push(v);
	}
	out
}

fn round2(data: &[u8]) -> Vec<u8> {
	let enc = rc4(&get_key_bytes(3), data);
	let mut_key = get_key_bytes(4);
	let pref_key = get_key_bytes(5);

	let mut out = Vec::with_capacity(enc.len() * 2);
	for (i, &b) in enc.iter().enumerate() {
		if i < 6 && i < pref_key.len() {
			out.push(pref_key[i]);
		}
		let mut v = b ^ get_mut_key(&mut_key, i);
		v = match i % 10 {
			0 | 8 => mut_c(v),
			1 => mut_b(v),
			2 | 6 => mut_dollar(v),
			3 => mut_h(v),
			4 | 9 => mut_s(v),
			5 => mut_k(v),
			7 => mut_underscore(v),
			_ => v,
		};
		out.push(v);
	}
	out
}

fn round3(data: &[u8]) -> Vec<u8> {
	let enc = rc4(&get_key_bytes(6), data);
	let mut_key = get_key_bytes(7);
	let pref_key = get_key_bytes(8);

	let mut out = Vec::with_capacity(enc.len() * 2);
	for (i, &b) in enc.iter().enumerate() {
		if i < 7 && i < pref_key.len() {
			out.push(pref_key[i]);
		}
		let mut v = b ^ get_mut_key(&mut_key, i);
		v = match i % 10 {
			0 => mut_c(v),
			1 => mut_f(v),
			2 | 8 => mut_s(v),
			3 => mut_g(v),
			4 => mut_y(v),
			5 => mut_m(v),
			6 => mut_dollar(v),
			7 => mut_k(v),
			9 => mut_b(v),
			_ => v,
		};
		out.push(v);
	}
	out
}

fn round4(data: &[u8]) -> Vec<u8> {
	let enc = rc4(&get_key_bytes(9), data);
	let mut_key = get_key_bytes(10);
	let pref_key = get_key_bytes(11);

	let mut out = Vec::with_capacity(enc.len() * 2);
	for (i, &b) in enc.iter().enumerate() {
		if i < 8 && i < pref_key.len() {
			out.push(pref_key[i]);
		}
		let mut v = b ^ get_mut_key(&mut_key, i);
		v = match i % 10 {
			0 => mut_b(v),
			1 | 9 => mut_m(v),
			2 | 7 => mut_l(v),
			3 | 5 => mut_s(v),
			4 | 6 => mut_underscore(v),
			8 => mut_y(v),
			_ => v,
		};
		out.push(v);
	}
	out
}

fn round5(data: &[u8]) -> Vec<u8> {
	let enc = rc4(&get_key_bytes(12), data);
	let mut_key = get_key_bytes(13);
	let pref_key = get_key_bytes(14);

	let mut out = Vec::with_capacity(enc.len() * 2);
	for (i, &b) in enc.iter().enumerate() {
		if i < 6 && i < pref_key.len() {
			out.push(pref_key[i]);
		}
		let mut v = b ^ get_mut_key(&mut_key, i);
		v = match i % 10 {
			0 => mut_underscore(v),
			1 | 7 => mut_s(v),
			2 => mut_c(v),
			3 | 5 => mut_m(v),
			4 => mut_b(v),
			6 => mut_f(v),
			8 => mut_dollar(v),
			9 => mut_g(v),
			_ => v,
		};
		out.push(v);
	}
	out
}

/// * `path`: API path, e.g. "/manga/some-hash/chapters"
/// * `body_size`: `encodeURIComponent(body).length` for POST, or 0 for GET
/// * `time`: 1 for GET manga requests, current millis for POST
pub fn generate_hash(path: &str, body_size: usize, time: i64) -> String {
	let base_string = format!("{path}:{body_size}:{time}");

	let encoded = encode_uri_component(&base_string)
		.replace("+", "%20")
		.replace("*", "%2A");

	let r1 = round1(encoded.as_bytes());
	let r2 = round2(&r1);
	let r3 = round3(&r2);
	let r4 = round4(&r3);
	let r5 = round5(&r4);

	URL_SAFE_NO_PAD.encode(r5)
}
