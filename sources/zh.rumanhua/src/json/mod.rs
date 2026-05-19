mod decrypt;

pub use decrypt::decrypt_params;

use aidoku::{
	Result,
	alloc::{String, Vec, string::ToString},
	error,
};
use serde::Deserialize;

pub fn extract_params(html: &str) -> Option<String> {
	let patterns = ["params = '", "params = \"", "params='", "params=\""];
	for pattern in patterns {
		if let Some(pos) = html.find(pattern) {
			let start = pos + pattern.len();
			let quote_char = pattern.chars().last()?;
			let sub = &html[start..];
			if let Some(end) = sub.find(quote_char) {
				return Some(sub[..end].to_string());
			}
		}
	}
	None
}

#[derive(Deserialize)]
struct ParamsJson {
	images: Vec<String>,
}

pub fn parse_images_from_html(html_str: &str) -> Result<Vec<String>> {
	let params_val = extract_params(html_str).ok_or_else(|| {
		let len = html_str.chars().count();
		let sample = html_str.chars().take(150).collect::<String>();
		error!("No params. Chars: {}. Preview: {}", len, sample)
	})?;

	let decrypted = decrypt_params(&params_val)
		.ok_or_else(|| error!("DecryptErr. Params len: {}", params_val.len()))?;

	let params_json: ParamsJson = serde_json::from_str(&decrypted)
		.map_err(|e| error!("ParseErr: {:?}. Decrypted: {}", e, decrypted))?;

	Ok(params_json.images)
}
