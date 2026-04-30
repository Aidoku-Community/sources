use aidoku::{
	AidokuError, Result,
	alloc::String,
	imports::net::Response,
	prelude::*,
};
use serde::de::DeserializeOwned;

pub trait ResponseJsonExt {
	fn parse_json<T: DeserializeOwned>(self) -> Result<T>;
}

impl ResponseJsonExt for Response {
	fn parse_json<T: DeserializeOwned>(self) -> Result<T> {
		let bytes = self.get_data()?;
		serde_json::from_slice::<T>(&bytes).map_err(|e| {
			let col = e.column();
			let start = col.saturating_sub(150);
			let context: String = core::str::from_utf8(&bytes)
				.unwrap_or("<non-utf8>")
				.chars()
				.skip(start)
				.take(300)
				.collect();
			AidokuError::message(format!(
				"JSON error at col {col} (body {} bytes): {e} | context: ...{context}...",
				bytes.len()
			))
		})
	}
}
