use aes::Aes128;
use aidoku::alloc::String;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use cbc::Decryptor;
use cbc::cipher::{BlockDecryptMut, KeyIvInit};

pub fn decrypt_params(encrypted_str: &str) -> Option<String> {
	let decoded = STANDARD.decode(encrypted_str.trim()).ok()?;
	if decoded.len() < 16 {
		return None;
	}
	let iv = &decoded[0..16];
	let ciphertext = &decoded[16..];
	let key = b"9S8$vJnU2ANeSRoF";

	let decryptor = Decryptor::<Aes128>::new_from_slices(key, iv).ok()?;
	let mut buf = ciphertext.to_vec();
	let decrypted_bytes = decryptor
		.decrypt_padded_mut::<cbc::cipher::block_padding::Pkcs7>(&mut buf)
		.ok()?;
	String::from_utf8(decrypted_bytes.to_vec()).ok()
}
