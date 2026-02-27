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
		let content = "03c707d0d37ee327dd01eb732eb5e05c:U2FsdGVkX18sztWigcz5Da6EkGImAVMrvbd4PAs74twEPrLZ0BjQKMg8lZr1dXo7wp922jpZ+/Vu7o02a6WCofNKYUqWtkRt7vKMr7zWS71m6rN2A4dGlAaDWhH90Bp1Z+ykFpr+dWjvexWuZ/l9O6z8tZO+ktZy2adjthBt4Ay+xlD3DGbxuK2gooDw3ILhDdGdq8W5HpNYd/34E0zZVs1cwCJsCMR8qFGgyDHhveGV9kqLqjdeWLDt2DYXZJGiTyJ1n7jPz/jnguVT+z7nglEzMvvMUn3aUK9tcJPHQhu84CBJTyGxSNysZqAB6SXg0rT8gu5Nma+ugATdQM5iao9ptxFn+P7UCYdQhHH9miOIjJYxEWOYeqFbYfjNBvh5nsCPvSjUSndyZdKn7Zq+xRIswA5maviRlZKF5yTGYcT3qchEhExc62BPuoZ/2ckteBLUcLiI+BaqbF5lou9vKXsmitgqhZCM9WF+wbOmfG65OakLYWTnGExhA7hivSrZdLY3MGNvXfANetm9q8V8Stv3FAhlfGUY/8MYcAkqT7FMewXY0hq4DrzuDQGthksFAP3dT/emmicupgHqh4XcDm0sNXEBXX5qG+rySMMZx8WwOhzu5GxSgsT/Af5+bx4Mv0ieEjCG0eal93oD0J3RDk4n7jmxdfnn47vipbKp/EJZhhVSOupt0udnyVbOtb/W";

		let (_, b64) = content.split_once(':').unwrap();

		let result = decrypt_cryptojs_passphrase(b64, crate::env::SECRET_DATA_CHAPTER);
		assert!(result.is_ok());
	}
}
