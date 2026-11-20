use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
use aidoku::{
	Result,
	alloc::{String, Vec},
	prelude::*,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::{Digest, Sha256};

// Both keys are in the site's reader bundle.
const SIGN_KEY: [u8; 32] = [
	0x85, 0xce, 0xdf, 0x4f, 0xed, 0x36, 0x02, 0xc3, 0xa3, 0x5c, 0x18, 0x01, 0x0b, 0x50, 0x25, 0x84,
	0xde, 0xe9, 0x59, 0x47, 0x08, 0xba, 0x67, 0x30, 0x52, 0xd5, 0x62, 0xc7, 0xf2, 0xb1, 0xa9, 0xde,
];
const SEAL_KEY: [u8; 32] = [
	0x10, 0xdd, 0x8d, 0xff, 0x99, 0x11, 0x00, 0x0f, 0x92, 0x86, 0xd3, 0xdf, 0xd8, 0xb9, 0xf7, 0x2b,
	0x3f, 0x76, 0x3c, 0x2f, 0x78, 0x3f, 0x89, 0xf5, 0xc3, 0x84, 0xcf, 0xf4, 0xa1, 0x19, 0x88, 0xac,
];

#[derive(Deserialize)]
struct Sealed {
	p: Vec<String>,
}

// The reader API refuses a request whose path and time it can't verify.
pub fn sign(path: &str, time: &str) -> String {
	let mut mac =
		<Hmac<Sha256> as Mac>::new_from_slice(&SIGN_KEY).expect("hmac accepts any key length");
	mac.update(format!("{path} {time}").as_bytes());
	URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}

// A 16-byte salt and a 12-byte IV precede the AES-GCM ciphertext of the page hashes.
pub fn unseal(sealed: &str) -> Result<Vec<String>> {
	let data = URL_SAFE_NO_PAD
		.decode(sealed.trim_end_matches('='))
		.map_err(|_| error!("Invalid sealed page list"))?;
	if data.len() < 28 {
		bail!("Invalid sealed page list");
	}
	let (salt, rest) = data.split_at(16);
	let (iv, ciphertext) = rest.split_at(12);
	let key = Sha256::new()
		.chain_update(SEAL_KEY)
		.chain_update(salt)
		.finalize();
	let plain = Aes256Gcm::new(&key)
		.decrypt(Nonce::from_slice(iv), ciphertext)
		.map_err(|_| error!("Failed to decrypt the page list"))?;
	Ok(serde_json::from_slice::<Sealed>(&plain)?.p)
}
