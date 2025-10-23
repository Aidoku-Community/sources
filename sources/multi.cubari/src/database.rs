use crate::settings;
use aidoku::{
	Manga, Result,
	alloc::{string::String, vec::Vec},
	imports::defaults::{DefaultValue, defaults_get, defaults_get_map, defaults_set},
	prelude::*,
};

const SERIES_KEY: &str = "history.series";

pub fn series_list() -> Vec<String> {
	defaults_get::<Vec<String>>(SERIES_KEY).unwrap_or_default()
}

pub fn add_or_update_manga(manga: &Manga) {
	if settings::get_save_series() {
		// add manga in index if it doesn't already exist
		let mut series = series_list();
		if !series.contains(&manga.key) {
			series.push(manga.key.clone());
			defaults_set(SERIES_KEY, DefaultValue::StringArray(series));
		}

		// update manga in index
		let key = format!("history.{}", manga.key);
		let mut map = defaults_get_map(&key).unwrap_or_default();
		map.insert("cover".into(), manga.cover.clone().unwrap_or_default());
		map.insert("title".into(), manga.title.clone());
		defaults_set(&key, DefaultValue::HashMap(map));
	}
}

pub fn get_manga<T: AsRef<str>>(key: T) -> Result<Manga> {
	let key = key.as_ref();
	let map = defaults_get_map(&format!("history.{key}")).ok_or(error!("Not in cache"))?;
	let cover = map.get("cover").cloned().ok_or(error!("Missing cover"))?;
	let title = map.get("title").cloned().ok_or(error!("Missing title"))?;
	Ok(Manga {
		key: key.into(),
		title,
		cover: Some(cover),
		..Default::default()
	})
}

pub fn delete_all_manga() {
	let series = series_list();
	for key in series {
		defaults_set(&format!("history.{key}"), DefaultValue::Null);
	}
	defaults_set(SERIES_KEY, DefaultValue::Null);
}
