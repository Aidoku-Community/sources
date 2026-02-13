use aes::{
	Aes128,
	cipher::{BlockDecryptMut, KeyIvInit, block_padding::NoPadding},
};
use aidoku::alloc::Vec;
use cbc::Decryptor;

type Aes128CbcDec = Decryptor<Aes128>;

pub fn decrypt_key_iv(message: &[u8], key: &[u8], iv: &[u8]) -> Option<Vec<u8>> {
	Aes128CbcDec::new_from_slices(key, iv)
		.ok()
		.and_then(|cipher| cipher.decrypt_padded_vec_mut::<NoPadding>(message).ok())
}
