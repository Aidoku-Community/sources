use crate::models::{MangaChapter, PageContainerResponse};
use aidoku::{HashMap, Result, alloc::string::String, alloc::string::ToString, prelude::*};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

pub fn resolve_ptr_table_json(table: &[Value], index: usize) -> Result<Value> {
	// This function will convert pointer-table encoded JSON format into normal JSON format.
	let value = &table[index];

	match value {
		// Object with a key and a value { _N: M } mappings.
		Value::Object(obj) => {
			let mut result = Map::new();

			for (k, v) in obj {
				// "_123" -> 123
				let Some(key_index) = k.strip_prefix('_').and_then(|i| i.parse::<usize>().ok())
				else {
					bail!("Invalid json key index")
				};

				let Some(key) = table[key_index].as_str() else {
					bail!("Invalid json key value")
				};

				let Some(value_index) = (match v {
					Value::Number(n) => n.as_i64(),
					_ => Some(-1),
				}) else {
					bail!("Invalid json value index")
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
				.map(|i| {
					if let Some(index) = i.as_i64() {
						if index >= 0 {
							if let Ok(result) = resolve_ptr_table_json(table, index as usize) {
								result
							} else {
								Value::Null
							}
						} else {
							Value::Null
						}
					} else {
						Value::Null
					}
				})
				.collect(),
		)),

		// Primitive value, just return as is.
		_ => Ok(value.clone()),
	}
}

pub fn to_json_data<T>(value: Value) -> Result<T>
where
	T: DeserializeOwned,
{
	// Example: {"pages/SearchPage":{"data":{...}}}
	let output: HashMap<String, PageContainerResponse<T>> = serde_json::from_value(value)?;
	let Some(data) = output.into_values().next() else {
		bail!("Input JSON do not match the required format")
	};
	Ok(data.data)
}

fn is_official_like(chapter: &MangaChapter) -> bool {
	chapter.group_id.is_some_and(|id| id == 17423)
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
