use crate::models::MangaChapter;
use aidoku::{
	HashMap, Result, alloc::string::String, alloc::string::ToString, alloc::vec::Vec, prelude::*,
};
use serde_json::{Map, Value};

pub fn resolve_ptr_table_json(table: &[Value], index: usize) -> Result<Value> {
	// This function will convert pointer-table encoded JSON format into normal JSON format.
	// Since the data format would most likely not have cycles, we didn't handle this inside here.
	let Some(value) = table.get(index) else {
		bail!("Invalid index")
	};

	match value {
		// Object with a key and a value { _N: M } mappings.
		Value::Object(obj) => {
			let mut result = Map::new();

			for (k, v) in obj {
				// "_123" -> 123
				let Ok(key_index) = k.trim_start_matches('_').parse::<usize>() else {
					bail!("Unable to convert key index to number")
				};

				let Some(key) = table.get(key_index).and_then(|v| v.as_str()) else {
					bail!("Unable to convert key value to string")
				};

				let Some(value_index) = v.as_i64() else {
					bail!("Unable to convert value index to number")
				};

				let resolved_value = if value_index >= 0 {
					resolve_ptr_table_json(table, value_index as usize)?
				} else {
					Value::Null
				};

				result.insert(key.into(), resolved_value);
			}

			Ok(Value::Object(result))
		}

		// If the value is an array, it would be an index array of any value.
		Value::Array(arr) => Ok(Value::Array(
			arr.iter()
				.map(|v| {
					let Some(index) = v.as_i64() else {
						bail!("Unable to convert index to number")
					};

					if index < 0 {
						Ok(Value::Null)
					} else {
						resolve_ptr_table_json(table, index as usize)
					}
				})
				.collect::<Result<Vec<Value>>>()?,
		)),

		// Primitive value, just return as is.
		_ => Ok(value.clone()),
	}
}

fn is_official_like(chapter: &MangaChapter) -> bool {
	let official_group_ids = [
		17423, // Official
		10712, // Manga Plus
		16861, // Viz Manga
		10110, // LINE Webtoon
		16168, // Tapas
		3521,  // Comikey
	];

	// There are probably others but tbh, they have not standardized this properly so this is
	// only a small chunk that I know of. Wait for the site to mature better before optimizing
	// this function. (And this only works for maybe 1% of the manga available)
	let official_scanlator_names = ["Official", "Official?", "MangaPlus", "Comikey"];

	let group_id = chapter
		.group_id
		.as_ref()
		.is_some_and(|id| official_group_ids.contains(id));

	let scanlator_name = chapter.scanlator_name.as_ref().is_some_and(|name| {
		official_scanlator_names
			.iter()
			.any(|s| s.to_lowercase() == name.to_lowercase())
	});

	group_id || scanlator_name
}

fn is_better(new: &MangaChapter, current: &MangaChapter) -> bool {
	let official_new = is_official_like(new);
	let official_cur = is_official_like(current);

	if official_new && !official_cur {
		return true;
	}
	if !official_new && official_cur {
		return false;
	}

	let new_created_at = new.created_at();
	let cur_created_at = current.created_at();
	new_created_at > cur_created_at
}

pub fn dedup_insert(map: &mut HashMap<String, MangaChapter>, chapter: MangaChapter) {
	let key = chapter.chapter_number.to_string();
	match map.get(&key) {
		None => {
			map.insert(key, chapter);
		}
		Some(current) => {
			if is_better(&chapter, current) {
				map.insert(key, chapter);
			}
		}
	}
}
