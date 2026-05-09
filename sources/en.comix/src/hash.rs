// reference: https://github.com/keiyoushi/extensions-source/blob/e6af5a11a7e8bdcfdfde50825b615e91dd2fc20c/src/en/comix/src/eu/kanade/tachiyomi/extension/en/comix/Hash.kt
use aidoku::{
	alloc::{string::String, vec::Vec},
	helpers::uri::encode_uri_component,
};
use base64::{
	Engine,
	engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};

// [RC4 key, mutKey, prefKey] × 5 rounds
const KEYS: [&str; 15] = [
	"JxTcdyiA5GZxnbrmthXBQfU2IMTKcY1+3nNhbq98Sgo=", // 0  RC4 key  round 1
	"3PordjODbhqla382Cxapmo/1JiABJQcjiJj1+48gTJ4=", // 1  mutKey   round 1
	"OaKvnI5ARA==",                                 // 2  prefKey  round 1
	"MHNBHYWA7lvy867fXgvGcJwWDk79KqUJUVFsh3RwnnI=", // 3  RC4 key  round 2
	"8i0Cru/VJBSVB2Y1GcMDVpzx2WepOcfnWdd81yxICl4=", // 4  mutKey   round 2
	"Fyskubz8VvA=",                                 // 5  prefKey  round 2
	"B46L1x+UeWP+19cRpQ+OZvdLAK9EHID8g3mSgn57tew=", // 6  RC4 key  round 3
	"DTSTmUt6LpDUw9r1lSQqyb3YlFTzruT8tk8wUGkwehQ=", // 7  mutKey   round 3
	"vY/meeI=",                                     // 8  prefKey  round 3
	"7xWfIF5THL5LAnRgAARg+4mjWHPU9n3PQwvzbaMNi+Q=", // 9  RC4 key  round 4
	"bewtiTuV+HJk56xxkf2iCljLgruCpBmN9BgE8i6gc9M=", // 10 mutKey   round 4
	"/Xcb2zAu8AU=",                                 // 11 prefKey  round 4
	"WgeCQ3T8R51uTwVSiVa7Zy0dN6JOg6Z5JleMS+HV8Aw=", // 12 RC4 key  round 5
	"yXayUVFrrcW56jQCEfZzuCidjpnWKjTDUNT7XeX9i7k=", // 13 mutKey   round 5
	"tSLco2w=",                                     // 14 prefKey  round 5
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
fn get_mut_key(mk: &[u8], idx: usize) -> u8 {
	if !mk.is_empty() && (idx % 32) < mk.len() {
		mk[idx % 32]
	} else {
		0
	}
}

#[inline]
fn op_shift_right7_left1(e: u8) -> u8 {
	e.rotate_left(1)
}
#[inline]
fn op_shift_left1_right7(e: u8) -> u8 {
	e.rotate_left(1)
}
#[inline]
fn op_shift_right2_left6(e: u8) -> u8 {
	e.rotate_right(2)
}
#[inline]
fn op_shift_left4_right4(e: u8) -> u8 {
	e.rotate_right(4)
}
#[inline]
fn op_shift_right4_left4(e: u8) -> u8 {
	e.rotate_left(4)
}

fn mutate(
	data: &[u8],
	mut_key: &[u8],
	pref_key: &[u8],
	pref_key_limit: usize,
	round: usize,
) -> Vec<u8> {
	let mut out = Vec::with_capacity(data.len() + pref_key_limit);
	for o in 0..data.len() {
		if o < pref_key_limit && o < pref_key.len() {
			out.push(pref_key[o]);
		}
		let mut n = data[o] ^ get_mut_key(mut_key, o);
		n = match round {
			1 => match o % 10 {
				0 => op_shift_right7_left1(n),
				1 => n ^ 37,
				2 => n ^ 81,
				3 => n ^ 147,
				4 => op_shift_right2_left6(n),
				5 | 8 => op_shift_right4_left4(n),
				6 => n ^ 218,
				7 => n.wrapping_add(159),
				9 => n ^ 180,
				_ => n,
			},
			2 => match o % 10 {
				0 | 9 => n ^ 180,
				1 => op_shift_left1_right7(n),
				2 => n ^ 147,
				3 => op_shift_right7_left1(n),
				4 => op_shift_right2_left6(n),
				5 => op_shift_right4_left4(n),
				6 | 8 => n.wrapping_add(159),
				7 => n.wrapping_add(34),
				_ => n,
			},
			3 => match o % 10 {
				0 => n ^ 81,
				1 => op_shift_right4_left4(n),
				2 | 9 => op_shift_left4_right4(n),
				3 => n ^ 37,
				4 => n.wrapping_add(159),
				5 => op_shift_left1_right7(n),
				6 => n ^ 180,
				7 => n.wrapping_add(34),
				8 => op_shift_right2_left6(n),
				_ => n,
			},
			4 => match o % 10 {
				0 | 7 => n ^ 218,
				1 | 4 => op_shift_left1_right7(n),
				2 => op_shift_right7_left1(n),
				3 => n.wrapping_add(159),
				5 | 8 => n ^ 180,
				6 => n ^ 147,
				9 => n ^ 37,
				_ => n,
			},
			5 => match o % 10 {
				0 => op_shift_left4_right4(n),
				1 | 3 => n ^ 147,
				2 => n.wrapping_add(34),
				4 | 9 => n ^ 218,
				5 | 7 => op_shift_left1_right7(n),
				6 => n ^ 180,
				8 => op_shift_right2_left6(n),
				_ => n,
			},
			_ => n,
		};
		out.push(n);
	}
	out
}

fn round1(data: &[u8]) -> Vec<u8> {
	let mutated = mutate(data, &get_key_bytes(1), &get_key_bytes(2), 7, 1);
	rc4(&get_key_bytes(0), &mutated)
}

fn round2(data: &[u8]) -> Vec<u8> {
	let mutated = mutate(data, &get_key_bytes(4), &get_key_bytes(5), 8, 2);
	rc4(&get_key_bytes(3), &mutated)
}

fn round3(data: &[u8]) -> Vec<u8> {
	let mutated = mutate(data, &get_key_bytes(7), &get_key_bytes(8), 5, 3);
	rc4(&get_key_bytes(6), &mutated)
}

fn round4(data: &[u8]) -> Vec<u8> {
	let mutated = mutate(data, &get_key_bytes(10), &get_key_bytes(11), 8, 4);
	rc4(&get_key_bytes(9), &mutated)
}

fn round5(data: &[u8]) -> Vec<u8> {
	let mutated = mutate(data, &get_key_bytes(13), &get_key_bytes(14), 5, 5);
	rc4(&get_key_bytes(12), &mutated)
}

/// * `path`: API path, e.g. "/manga/some-hash/chapters"
pub fn generate_hash(path: &str) -> String {
	let encoded = encode_uri_component(path)
		.replace("+", "%20")
		.replace("*", "%2A");

	let r1 = round1(encoded.as_bytes());
	let r2 = round2(&r1);
	let r3 = round3(&r2);
	let r4 = round4(&r3);
	let r5 = round5(&r4);

	URL_SAFE_NO_PAD.encode(r5)
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn test_manga_keys() {
		assert_eq!(
			generate_hash("/manga/prm8/chapters"),
			"bemJ-0y5bduXT9upsFZyqV4s6RO7JKSqdGy_wtVw2MsErBWwyqsxWwbL-D5qRSWCr15sWrYTsLd0-os"
		);
		assert_eq!(
			generate_hash("/chapters/7660562"),
			"bemJ-0y5bduXT9upsFZyqV4s6RO7JKSqdGy_wtVw2MsEpRV3yl8xub6LeSndTesynt7dDDNi"
		);
	}
}
