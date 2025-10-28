use aes::{
	Aes256,
	cipher::{BlockDecryptMut, KeyIvInit},
};
use aidoku::alloc::Vec;
use block_padding::Pkcs7;
use cbc::Decryptor;
use digest::{Digest, FixedOutputReset, HashMarker};
use md5::Md5;

type Aes256CbcDec = Decryptor<Aes256>;

const AES_128_LENGTH: usize = 16;
const AES_192_LENGTH: usize = 28;
const AES_256_LENGTH: usize = 32;

// references:
// https://github.com/Skittyblock/aidoku-community-sources/blob/d6a7fa423440f1ceee579402d61b1b56179689cf/src/rust/multi.batoto/src/crypto.rs
// https://github.com/keiyoushi/extensions-source/blob/main/lib/cryptoaes/src/main/java/eu/kanade/tachiyomi/lib/cryptoaes/CryptoAES.kt

pub fn decrypt_key_iv(message: &[u8], key: &[u8], iv: Option<&[u8; 16]>) -> Option<Vec<u8>> {
	let (salt, ciphertext) = if &message[0..=7] == b"Salted__" {
		(&message[8..=15], &message[16..])
	} else {
		("".as_bytes(), message)
	};

	let mut key_iv = [0; 32 + 16];
	let (actual_key, actual_iv) = if iv.is_none()
		|| (key.len() != AES_128_LENGTH
			&& key.len() != AES_192_LENGTH
			&& key.len() != AES_256_LENGTH)
	{
		evp_bytes_to_key::<Md5>(key, salt, 1, &mut key_iv);
		key_iv.split_at(32)
	} else {
		#[allow(clippy::unnecessary_unwrap)]
		(key as &[u8], iv.unwrap() as &[u8])
	};
	if let Ok(cipher) = Aes256CbcDec::new_from_slices(actual_key, actual_iv) {
		cipher.decrypt_padded_vec_mut::<Pkcs7>(ciphertext).ok()
	} else {
		None
	}
}

// https://github.com/openssl/openssl/blob/36614faa98c5a947a635d3f44e78c7c36b722534/crypto/evp/evp_key.c#L78
fn evp_bytes_to_key<D: Default + FixedOutputReset + HashMarker>(
	pass: &[u8],
	salt: &[u8],
	count: usize,
	output: &mut [u8],
) {
	let mut hasher = D::default();
	let mut derived_key = Vec::with_capacity(output.len());
	let mut block = Vec::new();

	while derived_key.len() < output.len() {
		if !block.is_empty() {
			hasher.update(&block);
		}
		hasher.update(pass);
		hasher.update(salt.as_ref());
		block = hasher.finalize_reset().to_vec();

		for _ in 1..count {
			hasher.update(&block);
			block = hasher.finalize_reset().to_vec();
		}

		derived_key.extend_from_slice(&block);
	}

	output.copy_from_slice(&derived_key[0..output.len()]);
}

#[cfg(test)]
mod test {
	use super::*;
	use aidoku_test::*;

	use aidoku::alloc::String;
	use base64::prelude::*;

	// https://stackoverflow.com/a/41434590
	#[aidoku_test]
	fn test_decrypt() {
		let cipher_text = "U2FsdGVkX1+tsmZvCEFa/iGeSA0K7gvgs9KXeZKwbCDNCs2zPo+BXjvKYLrJutMK+hxTwl/hyaQLOaD7LLIRo2I5fyeRMPnroo6k8N9uwKk=";
		let secret = "René Über";

		let decoded = BASE64_STANDARD
			.decode(cipher_text)
			.expect("failed base64 decode");
		let result = decrypt_key_iv(&decoded, secret.as_bytes(), None).expect("failed decrypt");
		let text = String::from_utf8(result).expect("failed utf8 decode");

		assert_eq!(text, "The quick brown fox jumps over the lazy dog. 👻 👻");
	}
}
