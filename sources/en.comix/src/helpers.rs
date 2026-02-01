use aidoku::{
	HashMap,
	alloc::{
		fmt,
		string::{String, ToString},
	},
};
use serde::{
	Deserializer,
	de::{self, Visitor},
};

use crate::models::ComixChapter;

fn is_official_like(ch: &ComixChapter) -> bool {
	ch.scanlation_group_id == 9275 || ch.is_official == 1
}

fn is_better(new_ch: &ComixChapter, cur: &ComixChapter) -> bool {
	let official_new = is_official_like(new_ch);
	let official_cur = is_official_like(cur);

	if official_new && !official_cur {
		return true;
	}
	if !official_new && official_cur {
		return false;
	}

	if new_ch.votes > cur.votes {
		return true;
	}
	if new_ch.votes < cur.votes {
		return false;
	}

	new_ch.updated_at > cur.updated_at
}

pub fn dedup_insert(map: &mut HashMap<String, ComixChapter>, ch: ComixChapter) {
	let key = ch.number.to_string();
	match map.get(&key) {
		None => {
			map.insert(key, ch);
		}
		Some(current) => {
			if is_better(&ch, current) {
				map.insert(key, ch);
			}
		}
	}
}

pub fn de_safe_int_bool<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
	D: Deserializer<'de>,
{
	struct V;

	impl<'de> Visitor<'de> for V {
		type Value = i32;

		fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
			f.write_str("bool/int/string for is_official")
		}

		fn visit_bool<E>(self, v: bool) -> Result<i32, E>
		where
			E: de::Error,
		{
			Ok(if v { 1 } else { 0 })
		}

		fn visit_i64<E>(self, v: i64) -> Result<i32, E>
		where
			E: de::Error,
		{
			Ok(if v != 0 { 1 } else { 0 })
		}

		fn visit_u64<E>(self, v: u64) -> Result<i32, E>
		where
			E: de::Error,
		{
			Ok(if v != 0 { 1 } else { 0 })
		}

		fn visit_str<E>(self, v: &str) -> Result<i32, E>
		where
			E: de::Error,
		{
			let s = v.trim();
			if s.eq_ignore_ascii_case("true") {
				return Ok(1);
			}
			if s.eq_ignore_ascii_case("false") {
				return Ok(0);
			}
			Ok(s.parse::<i32>().unwrap_or(0).clamp(0, 1))
		}

		fn visit_string<E>(self, v: String) -> Result<i32, E>
		where
			E: de::Error,
		{
			self.visit_str(&v)
		}

		fn visit_none<E>(self) -> Result<i32, E>
		where
			E: de::Error,
		{
			Ok(0)
		}

		fn visit_unit<E>(self) -> Result<i32, E>
		where
			E: de::Error,
		{
			Ok(0)
		}
	}

	deserializer.deserialize_any(V)
}
