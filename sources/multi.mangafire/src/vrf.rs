// original script by @Trung0246 on github
// based on https://github.com/keiyoushi/extensions-source/blob/main/src/all/mangafire/src/eu/kanade/tachiyomi/extension/all/mangafire/VrfGenerator.kt
use aidoku::{
	alloc::{string::String, vec, vec::Vec},
	helpers::uri::encode_uri_component,
	HashMap,
};
use base64::{engine::general_purpose, Engine as _};

pub struct VrfGenerator;

impl VrfGenerator {
	fn atob(data: &str) -> Vec<u8> {
		general_purpose::STANDARD.decode(data).unwrap()
	}

	fn btoa(data: &[u8]) -> String {
		general_purpose::STANDARD.encode(data)
	}

	fn rc4(key: &[u8], input: &[u8]) -> Vec<u8> {
		let mut s: Vec<u8> = (0..=255).collect();
		let mut j = 0usize;

		// KSA
		for i in 0..256 {
			j = (j + s[i] as usize + key[i % key.len()] as usize) & 0xFF;
			s.swap(i, j);
		}

		// PRGA
		let mut output = vec![0u8; input.len()];
		let mut i = 0usize;
		j = 0usize;
		for (y, &inp) in input.iter().enumerate() {
			i = (i + 1) & 0xFF;
			j = (j + s[i] as usize) & 0xFF;
			s.swap(i, j);
			let k = s[(s[i] as usize + s[j] as usize) & 0xFF];
			output[y] = inp ^ k;
		}
		output
	}

	fn transform(
		input: &[u8],
		init_seed_bytes: &[u8],
		prefix_key_bytes: &[u8],
		prefix_len: usize,
		schedule: &[fn(u8) -> u8],
	) -> Vec<u8> {
		let mut out = Vec::new();
		for i in 0..input.len() {
			if i < prefix_len {
				out.push(prefix_key_bytes[i]);
			}
			let transformed = schedule[i % 10](input[i] ^ init_seed_bytes[i % 32]);
			out.push(transformed);
		}
		out
	}

	pub fn generate(input: &str) -> String {
		let rc4_keys: HashMap<&str, &str> = [
			("l", "u8cBwTi1CM4XE3BkwG5Ble3AxWgnhKiXD9Cr279yNW0="),
			("g", "t00NOJ/Fl3wZtez1xU6/YvcWDoXzjrDHJLL2r/IWgcY="),
			("B", "S7I+968ZY4Fo3sLVNH/ExCNq7gjuOHjSRgSqh6SsPJc="),
			("m", "7D4Q8i8dApRj6UWxXbIBEa1UqvjI+8W0UvPH9talJK8="),
			("F", "0JsmfWZA1kwZeWLk5gfV5g41lwLL72wHbam5ZPfnOVE="),
		]
		.iter()
		.cloned()
		.collect();

		let seeds32: HashMap<&str, &str> = [
			("A", "pGjzSCtS4izckNAOhrY5unJnO2E1VbrU+tXRYG24vTo="),
			("V", "dFcKX9Qpu7mt/AD6mb1QF4w+KqHTKmdiqp7penubAKI="),
			("N", "owp1QIY/kBiRWrRn9TLN2CdZsLeejzHhfJwdiQMjg3w="),
			("P", "H1XbRvXOvZAhyyPaO68vgIUgdAHn68Y6mrwkpIpEue8="),
			("k", "2Nmobf/mpQ7+Dxq1/olPSDj3xV8PZkPbKaucJvVckL0="),
		]
		.iter()
		.cloned()
		.collect();

		let prefix_keys: HashMap<&str, &str> = [
			("O", "Rowe+rg/0g=="),
			("v", "8cULcnOMJVY8AA=="),
			("L", "n2+Og2Gth8Hh"),
			("p", "aRpvzH+yoA=="),
			("W", "ZB4oBi0="),
		]
		.iter()
		.cloned()
		.collect();

		// Schedules
		let schedule_c: [fn(u8) -> u8; 10] = [
			|c| c.wrapping_sub(48),
			|c| c.wrapping_sub(19),
			|c| c ^ 241,
			|c| c.wrapping_sub(19),
			|c| c.wrapping_add(223),
			|c| c.wrapping_sub(19),
			|c| c.wrapping_sub(170),
			|c| c.wrapping_sub(19),
			|c| c.wrapping_sub(48),
			|c| c ^ 8,
		];
		let schedule_y: [fn(u8) -> u8; 10] = [
			|c| c.rotate_right(4),
			|c| c.wrapping_add(223),
			|c| c.rotate_right(4),
			|c| c ^ 163,
			|c| c.wrapping_sub(48),
			|c| c.wrapping_add(82),
			|c| c.wrapping_add(223),
			|c| c.wrapping_sub(48),
			|c| c ^ 83,
			|c| c.rotate_right(4),
		];
		let schedule_b: [fn(u8) -> u8; 10] = [
			|c| c.wrapping_sub(19),
			|c| c.wrapping_add(82),
			|c| c.wrapping_sub(48),
			|c| c.wrapping_sub(170),
			|c| c.rotate_right(4),
			|c| c.wrapping_sub(48),
			|c| c.wrapping_sub(170),
			|c| c ^ 8,
			|c| c.wrapping_add(82),
			|c| c ^ 163,
		];
		let schedule_j: [fn(u8) -> u8; 10] = [
			|c| c.wrapping_add(223),
			|c| c.rotate_right(4),
			|c| c.wrapping_add(223),
			|c| c ^ 83,
			|c| c.wrapping_sub(19),
			|c| c.wrapping_add(223),
			|c| c.wrapping_sub(170),
			|c| c.wrapping_add(223),
			|c| c.wrapping_sub(170),
			|c| c ^ 83,
		];
		let schedule_e: [fn(u8) -> u8; 10] = [
			|c| c.wrapping_add(82),
			|c| c ^ 83,
			|c| c ^ 163,
			|c| c.wrapping_add(82),
			|c| c.wrapping_sub(170),
			|c| c ^ 8,
			|c| c ^ 241,
			|c| c.wrapping_add(82),
			|c| c.wrapping_add(176),
			|c| c.rotate_right(4),
		];

		let input = encode_uri_component(input);
		let mut bytes = input.as_bytes().to_vec();
		bytes = Self::rc4(&Self::atob(rc4_keys["l"]), &bytes);
		bytes = Self::transform(
			&bytes,
			&Self::atob(seeds32["A"]),
			&Self::atob(prefix_keys["O"]),
			7,
			&schedule_c,
		);
		bytes = Self::rc4(&Self::atob(rc4_keys["g"]), &bytes);
		bytes = Self::transform(
			&bytes,
			&Self::atob(seeds32["V"]),
			&Self::atob(prefix_keys["v"]),
			10,
			&schedule_y,
		);
		bytes = Self::rc4(&Self::atob(rc4_keys["B"]), &bytes);
		bytes = Self::transform(
			&bytes,
			&Self::atob(seeds32["N"]),
			&Self::atob(prefix_keys["L"]),
			9,
			&schedule_b,
		);
		bytes = Self::rc4(&Self::atob(rc4_keys["m"]), &bytes);
		bytes = Self::transform(
			&bytes,
			&Self::atob(seeds32["P"]),
			&Self::atob(prefix_keys["p"]),
			7,
			&schedule_j,
		);
		bytes = Self::rc4(&Self::atob(rc4_keys["F"]), &bytes);
		bytes = Self::transform(
			&bytes,
			&Self::atob(seeds32["k"]),
			&Self::atob(prefix_keys["W"]),
			5,
			&schedule_e,
		);

		let mut encoded = Self::btoa(&bytes);
		encoded = encoded.replace("+", "-").replace("/", "_").replace("=", "");
		encoded
	}
}

#[cfg(test)]
mod test {
	use super::VrfGenerator;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn test_vrf() {
		assert_eq!(
			VrfGenerator::generate("67890@ The quick brown fox jumps over the lazy dog @12345"),
			"ZBYeRCjYBk0tkZnKW4kTuWBYw-81e-csvu6v17UY4zchviixt67VJ\
			 _tjpFEsOXB-a8X4ZFpDoDbPq8ms-7IyN95vmLVdP5vWSoTAl4ZbIB\
			 E8xijci8emrkdEYmArOPMUq5KAc3KEabUzHkNwjBtwvs0fQR7nDpI"
		)
	}
}
