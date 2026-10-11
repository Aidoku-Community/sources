use crate::{database, settings};
use aidoku::{
	Manga, Result,
	alloc::{string::String, vec::Vec},
	imports::net::Request,
	prelude::*,
};

pub struct RemoteStorage {
	url: String,
	token: String,
}

impl RemoteStorage {
	pub fn new() -> Option<Self> {
		let url = settings::get_storage_url();
		let token = settings::get_token();
		if url.is_empty() || token.is_empty() {
			return None;
		}
		Some(Self { url, token })
	}

	pub fn get_all_series(&self) -> Result<Vec<Manga>> {
		let mut response = Request::get(format!("{}/cubari/", self.url))?
			.header("Authorization", &format!("Bearer {}", self.token))
			.send()?;
		if response.status_code() == 401 {
			settings::set_token("");
			bail!("Unauthorized: Log in again to access history")
		}

		let json: serde_json::Value = response.get_json()?;
		let items = json.get("items").ok_or(error!("Missing `items` field"))?;
		let series = items
			.get("series/")
			.ok_or(error!("Missing `series/` field"))?;
		let revision = series
			.get("ETag")
			.and_then(|f| f.as_str())
			.ok_or(error!("Missing `ETag` field"))?;

		let mut series_list = database::series_list()
			.iter()
			.filter_map(|series| database::get_manga(series).ok())
			.collect::<Vec<_>>();

		if settings::get_history_revision() == revision {
			// database is already up to date with remote storage
			Ok(series_list)
		} else {
			let json: serde_json::Value = Request::get(format!("{}/cubari/series/", self.url))?
				.header("Authorization", &format!("Bearer {}", self.token))
				.json_owned()?;
			let items = json
				.get("items")
				.and_then(|v| v.as_object())
				.ok_or(error!("Missing `items` field"))?;

			for id in items.keys() {
				let key = id.replace('-', "/");
				if !series_list.iter().any(|series| series.key == key) {
					// fetch missing series data
					let series: serde_json::Value =
						Request::get(format!("{}/cubari/series/{id}", self.url))?
							.header("Authorization", &format!("Bearer {}", self.token))
							.json_owned()?;
					let title = series
						.get("title")
						.and_then(|v| v.as_str())
						.ok_or(error!("Missing `title` field"))?
						.into();
					let cover = series
						.get("coverUrl")
						.and_then(|v| v.as_str())
						.map(|s| s.into());

					let new_manga = Manga {
						key,
						title,
						cover,
						..Default::default()
					};
					database::add_or_update_manga(&new_manga);
					series_list.push(new_manga);
				}
			}

			settings::set_history_revision(revision);

			Ok(series_list)
		}
	}
}
