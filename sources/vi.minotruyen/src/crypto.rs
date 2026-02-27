// CryptoJS "Salted__" AES-256-CBC decrypt (passphrase) Misakiiiii

use aes::Aes256;
use aes::cipher::{BlockDecrypt, KeyInit};
use aidoku::alloc::string::String;
use aidoku::alloc::vec::Vec;
use anyhow::{Result, bail};
use base64::engine::general_purpose;
use base64::*;
use block_padding::generic_array::GenericArray;

fn evp_bytes_to_key(password: &[u8], salt: &[u8]) -> (Vec<u8>, Vec<u8>) {
	let mut out = Vec::new();
	let mut prev: Vec<u8> = Vec::new();

	while out.len() < 48 {
		let mut buf = Vec::new();
		buf.extend_from_slice(&prev);
		buf.extend_from_slice(password);
		buf.extend_from_slice(salt);

		prev = md5::compute(&buf).0.to_vec();
		out.extend_from_slice(&prev);
	}

	let key = out[..32].to_vec();
	let iv = out[32..48].to_vec();

	// println!("{:?}", key);

	(key, iv)
}
// CBC decrypt (手動実装)
fn aes256_cbc_decrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>> {
	let cipher = Aes256::new_from_slice(key).map_err(|e| anyhow::anyhow!("{}", e))?;
	let mut out = crate::vec![0u8; data.len()];

	let mut prev = iv.to_vec();

	for (i, chunk) in data.chunks_exact(16).enumerate() {
		let mut block = GenericArray::clone_from_slice(chunk);

		// 解読
		cipher.decrypt_block(&mut block);

		// XOR with previous
		for j in 0..16 {
			block[j] ^= prev[j];
		}

		// 出力へ
		out[i * 16..i * 16 + 16].copy_from_slice(&block);

		prev.copy_from_slice(chunk);
	}

	Ok(out)
}

// PKCS7 アンパディング
fn pkcs7_unpad(data: &[u8]) -> Option<Vec<u8>> {
	let pad = *data.last()? as usize;
	if pad == 0 || pad > 16 || pad > data.len() {
		return None;
	}
	for &b in &data[data.len() - pad..] {
		if b as usize != pad {
			return None;
		}
	}
	Some(data[..data.len() - pad].to_vec())
}

// CryptoJS decrypt 互換
pub fn decrypt_cryptojs_passphrase(b64: &str, password: &str) -> Result<String> {
	let raw = general_purpose::STANDARD
		.decode(b64)
		.map_err(|e| anyhow::anyhow!(e))?;

	if raw.len() < 16 || &raw[..8] != b"Salted__" {
		// return None;
		bail!("Invalid data");
	}

	let salt = &raw[8..16];
	let data = &raw[16..];

	// OpenSSL 互換 key, iv
	let (key, iv) = evp_bytes_to_key(password.as_bytes(), salt);

	// AES CBC decrypt
	let decrypted = aes256_cbc_decrypt(&key, &iv, data)?;

	// PKCS7 remove
	let unpadded = pkcs7_unpad(&decrypted).ok_or(anyhow::anyhow!("pkcs7_unpad failed"))?;

	String::from_utf8(unpadded).map_err(|e| anyhow::anyhow!(e))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[aidoku_test::aidoku_test]
	fn test_decrypt() {
		let content = "9e1d295315a338ab182baa30083b2ad7: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)xLvlKveivSEacwCUqVIn0NNCYABFoAvF0Ym/nDYAA_";

		let (_, b64) = content.split_once(':').unwrap();

		let result = decrypt_cryptojs_passphrase(b64, crate::env::SECRET_DATA_CHAPTER);
		assert!(result.is_ok());
	}
}
