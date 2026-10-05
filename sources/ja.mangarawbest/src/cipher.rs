use aes_gcm::{
	Aes256Gcm, KeyInit, Nonce,
	aead::{Aead, Payload},
};
use aidoku::{
	Result,
	alloc::{String, Vec},
	prelude::*,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use sha2::Sha256;

// Both keys are embedded in the site's obfuscated /js/bookmark.js and change
// whenever that decoder is rotated.
const PATH_KEY: [u8; 32] = [
	0x26, 0x55, 0x6e, 0x52, 0x42, 0x4c, 0x79, 0x3a, 0x24, 0xb5, 0x5a, 0x5f, 0x36, 0x62, 0x2a, 0xe7,
	0x77, 0x0a, 0x87, 0xfb, 0xc3, 0x91, 0x28, 0x66, 0xd2, 0xf8, 0x1a, 0x75, 0x8d, 0x75, 0x8c, 0x73,
];
const TOKEN_KEY: [u8; 32] = [
	0xaa, 0x86, 0x23, 0x9f, 0xab, 0x5b, 0x99, 0x4e, 0x05, 0x72, 0x00, 0x46, 0xa3, 0xde, 0x8e, 0x2e,
	0x27, 0x6f, 0x48, 0x8d, 0x06, 0xa1, 0xec, 0xfb, 0x90, 0x03, 0xe7, 0xfc, 0x9e, 0x29, 0x5f, 0x73,
];
const TOKEN_VERSION: u8 = 1;

fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
	let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("hmac accepts any key length");
	mac.update(data);
	mac.finalize().into_bytes().into()
}

pub fn sign_path(path: &str) -> String {
	let mac = hmac_sha256(&PATH_KEY, path.as_bytes());
	let mut token = [TOKEN_VERSION; 17];
	token[1..].copy_from_slice(&mac[..16]);
	URL_SAFE_NO_PAD.encode(token)
}

// The payload is a 12-byte IV followed by the AES-GCM ciphertext, and the
// chapter path is bound to it as associated data.
pub fn decrypt_pages(path: &str, token: &str, encoded: &str) -> Result<Vec<u8>> {
	let data = URL_SAFE_NO_PAD
		.decode(encoded)
		.map_err(|_| error!("invalid page payload"))?;
	let (iv, ciphertext) = data
		.split_at_checked(12)
		.ok_or_else(|| error!("truncated page payload"))?;

	let key = hmac_sha256(&TOKEN_KEY, token.as_bytes());
	Aes256Gcm::new(&key.into())
		.decrypt(
			Nonce::from_slice(iv),
			Payload {
				msg: ciphertext,
				aad: path.as_bytes(),
			},
		)
		.map_err(|_| error!("failed to decrypt page list"))
}

#[cfg(test)]
mod test {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn test_sign_path() {
		// tokens the site's own reader requested for these chapters
		assert_eq!(
			sign_path("wu-lun-nonu-shen-sama-nadeshikoliao-nomedarugohan/di-127hua"),
			"AZ019wm5U6P2Dr9LvECINCA"
		);
		assert_eq!(
			sign_path("tu-long-nobai/di-989hua"),
			"AVq22ojJoPdnb_qQ141QiTI"
		);
	}
}
