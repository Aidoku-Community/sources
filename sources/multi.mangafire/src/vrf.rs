// original script by @Trung0246 on github
// updated with keys from @podimium on discord
// based on https://github.com/keiyoushi/extensions-source/blob/5a08b6078384a90ed23cc084d8bfc7b6f8397f07/src/all/mangafire/src/eu/kanade/tachiyomi/extension/all/mangafire/VrfGenerator.kt
use aidoku::{
	alloc::{string::String, vec, vec::Vec},
	helpers::uri::encode_uri_component,
};
use base64::{Engine, engine::general_purpose};

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
		let rc4_keys: [&str; 5] = [
			// "u8cBwTi1CM4XE3BkwG5Ble3AxWgnhKiXD9Cr279yNW0=",
			"FgxyJUQDPUGSzwbAq/ToWn4/e8jYzvabE+dLMb1XU1o=",
			// "t00NOJ/Fl3wZtez1xU6/YvcWDoXzjrDHJLL2r/IWgcY=",
			"CQx3CLwswJAnM1VxOqX+y+f3eUns03ulxv8Z+0gUyik=",
			// "S7I+968ZY4Fo3sLVNH/ExCNq7gjuOHjSRgSqh6SsPJc=",
			"fAS+otFLkKsKAJzu3yU+rGOlbbFVq+u+LaS6+s1eCJs=",
			// "7D4Q8i8dApRj6UWxXbIBEa1UqvjI+8W0UvPH9talJK8=",
			"Oy45fQVK9kq9019+VysXVlz1F9S1YwYKgXyzGlZrijo=",
			// "0JsmfWZA1kwZeWLk5gfV5g41lwLL72wHbam5ZPfnOVE=",
			"aoDIdXezm2l3HrcnQdkPJTDT8+W6mcl2/02ewBHfPzg=",
		];

		let seeds32: [&str; 5] = [
			// "pGjzSCtS4izckNAOhrY5unJnO2E1VbrU+tXRYG24vTo=",
			"yH6MXnMEcDVWO/9a6P9W92BAh1eRLVFxFlWTHUqQ474=",
			// "dFcKX9Qpu7mt/AD6mb1QF4w+KqHTKmdiqp7penubAKI=",
			"RK7y4dZ0azs9Uqz+bbFB46Bx2K9EHg74ndxknY9uknA=",
			// "owp1QIY/kBiRWrRn9TLN2CdZsLeejzHhfJwdiQMjg3w=",
			"rqr9HeTQOg8TlFiIGZpJaxcvAaKHwMwrkqojJCpcvoc=",
			// "H1XbRvXOvZAhyyPaO68vgIUgdAHn68Y6mrwkpIpEue8=",
			"/4GPpmZXYpn5RpkP7FC/dt8SXz7W30nUZTe8wb+3xmU=",
			// "2Nmobf/mpQ7+Dxq1/olPSDj3xV8PZkPbKaucJvVckL0=",
			"wsSGSBXKWA9q1oDJpjtJddVxH+evCfL5SO9HZnUDFU8=",
		];

		let prefix_keys: [&str; 5] = [
			// "Rowe+rg/0g==",
			"l9PavRg=",
			// "8cULcnOMJVY8AA==",
			"Ml2v7ag1Jg==",
			// "n2+Og2Gth8Hh",
			"i/Va0UxrbMo=",
			// "aRpvzH+yoA==",
			"WFjKAHGEkQM=",
			// "ZB4oBi0=",
			"5Rr27rWd",
		];

		let schedule_0: [fn(u8) -> u8; 10] = [
			|c| c.wrapping_sub(223),
			|c| c.rotate_right(4),
			|c| c.rotate_right(4),
			|c| c.wrapping_add(234),
			|c| c.rotate_right(7),
			|c| c.rotate_right(2),
			|c| c.rotate_right(7),
			|c| c.wrapping_sub(223),
			|c| c.rotate_right(7),
			|c| c.rotate_right(6),
		];
		let schedule_1: [fn(u8) -> u8; 10] = [
			|c| c.wrapping_add(19),
			|c| c.rotate_right(7),
			|c| c.wrapping_add(19),
			|c| c.rotate_right(6),
			|c| c.wrapping_add(19),
			|c| c.rotate_right(1),
			|c| c.wrapping_add(19),
			|c| c.rotate_right(6),
			|c| c.rotate_right(7),
			|c| c.rotate_right(4),
		];
		let schedule_2: [fn(u8) -> u8; 10] = [
			|c| c.wrapping_sub(223),
			|c| c.rotate_right(1),
			|c| c.wrapping_add(19),
			|c| c.wrapping_sub(223),
			|c| c.rotate_left(2),
			|c| c.wrapping_sub(223),
			|c| c.wrapping_add(19),
			|c| c.rotate_left(1),
			|c| c.rotate_left(2),
			|c| c.rotate_left(1),
		];
		let schedule_3: [fn(u8) -> u8; 10] = [
			|c| c.wrapping_add(19),
			|c| c.rotate_left(1),
			|c| c.rotate_left(1),
			|c| c.rotate_right(1),
			|c| c.wrapping_add(234),
			|c| c.rotate_left(1),
			|c| c.wrapping_sub(223),
			|c| c.rotate_left(6),
			|c| c.rotate_left(4),
			|c| c.rotate_left(1),
		];
		let schedule_4: [fn(u8) -> u8; 10] = [
			|c| c.rotate_right(1),
			|c| c.rotate_left(1),
			|c| c.rotate_left(6),
			|c| c.rotate_right(1),
			|c| c.rotate_left(2),
			|c| c.rotate_right(4),
			|c| c.rotate_left(1),
			|c| c.rotate_left(1),
			|c| c.wrapping_sub(223),
			|c| c.rotate_left(2),
		];

		let input = encode_uri_component(input);
		let mut bytes = input.as_bytes().to_vec();

		bytes = Self::rc4(&Self::atob(rc4_keys[0]), &bytes);
		bytes = Self::transform(
			&bytes,
			&Self::atob(seeds32[0]),
			&Self::atob(prefix_keys[0]),
			Self::atob(prefix_keys[0]).len(),
			&schedule_0,
		);
		bytes = Self::rc4(&Self::atob(rc4_keys[1]), &bytes);
		bytes = Self::transform(
			&bytes,
			&Self::atob(seeds32[1]),
			&Self::atob(prefix_keys[1]),
			Self::atob(prefix_keys[1]).len(),
			&schedule_1,
		);
		bytes = Self::rc4(&Self::atob(rc4_keys[2]), &bytes);
		bytes = Self::transform(
			&bytes,
			&Self::atob(seeds32[2]),
			&Self::atob(prefix_keys[2]),
			Self::atob(prefix_keys[2]).len(),
			&schedule_2,
		);
		bytes = Self::rc4(&Self::atob(rc4_keys[3]), &bytes);
		bytes = Self::transform(
			&bytes,
			&Self::atob(seeds32[3]),
			&Self::atob(prefix_keys[3]),
			Self::atob(prefix_keys[3]).len(),
			&schedule_3,
		);
		bytes = Self::rc4(&Self::atob(rc4_keys[4]), &bytes);
		bytes = Self::transform(
			&bytes,
			&Self::atob(seeds32[4]),
			&Self::atob(prefix_keys[4]),
			Self::atob(prefix_keys[4]).len(),
			&schedule_4,
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
			// "ZBYeRCjYBk0tkZnKW4kTuWBYw-81e-csvu6v17UY4zchviixt67VJ\
			//  _tjpFEsOXB-a8X4ZFpDoDbPq8ms-7IyN95vmLVdP5vWSoTAl4ZbIB\
			//  E8xijci8emrkdEYmArOPMUq5KAc3KEabUzHkNwjBtwvs0fQR7nDpI"
			"5fcaUfZo7rW1-Z3vTEvXO5sJBfP2zuTM2NIVmftpuGhYgy8c-Yl92\
			 uQOuxzYksgVMUWKu7h-Pt5_6c0KZ2c1BpRQwVCIkRycge1pensQ__\
			 YViJZddxqB5PvElml6UdQ1h4w8kCFftPUYNoSHTqNBX0HfFg"
		)
	}
}
