use crate::{
	LOGIN_COOKIE_KEY, models::MangaChapter, models::PageContainer, models::UserProfile,
	settings::get_deduped_group_list, settings::get_login_cookie,
};
use aidoku::{
	HashMap, Result,
	alloc::{
		string::{String, ToString},
		vec::Vec,
	},
	imports::net::{Request, Response},
	prelude::*,
};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

const CF_CHALLENGE_ERROR_MESSAGE: &str = "Response returned CF challenge page instead of JSON data. If problem persist, please clear the source cache and restart the application to resolve this issue.";

fn create_request_get(url: &str) -> Result<Request> {
	let mut request = Request::get(url)?;
	if let Some(token) = get_login_cookie() {
		request = request.header("Cookie", &format!("{LOGIN_COOKIE_KEY}={token}"));
	}
	Ok(request)
}

fn response_is_ok(response: &Response) -> Result<()> {
	if response
		.get_header("cf-mitigated")
		.is_some_and(|value| value == "challenge")
	{
		bail!("{CF_CHALLENGE_ERROR_MESSAGE}")
	} else if response.status_code() >= 400 {
		bail!("Response Error: {}", response.status_code())
	}
	Ok(())
}

pub fn get_json_data<T>(url: &str) -> Result<T>
where
	T: DeserializeOwned,
{
	let request = create_request_get(url)?;
	let response = request.send()?;
	response_is_ok(&response)?;
	response.get_json_owned::<T>()
}

pub fn get_page_container_json_data<T>(url: &str) -> Result<T>
where
	T: DeserializeOwned,
{
	let request = create_request_get(url)?;
	let response = request.send()?;
	response_is_ok(&response)?;
	handle_page_container_json_data_response(&response)
}

pub fn get_bulk_page_container_json_data<T>(urls: &[String]) -> Result<Vec<T>>
where
	T: DeserializeOwned,
{
	let mut result = Vec::<T>::with_capacity(urls.len());
	let mut requests = Vec::<Request>::with_capacity(urls.len());

	for url in urls.iter() {
		let request = create_request_get(url)?;
		requests.push(request)
	}

	let responses = Request::send_all(requests);

	for response_result in responses.into_iter() {
		let response = response_result?;
		response_is_ok(&response)?;
		let data = handle_page_container_json_data_response(&response)?;
		result.push(data);
	}

	Ok(result)
}

fn handle_page_container_json_data_response<T>(response: &Response) -> Result<T>
where
	T: DeserializeOwned,
{
	let ptr_table_json = serde_json::from_slice::<Vec<Value>>(&response.get_data()?)?;
	let json = resolve_ptr_table_json(&ptr_table_json, 0)?;
	let Ok(page_container_json) = serde_json::from_value::<HashMap<String, PageContainer<T>>>(json)
	else {
		bail!("Invalid JSON data. Expected an object with page container data.")
	};
	let Some(page_container) = page_container_json.into_values().next() else {
		bail!("Page container data does not exists.")
	};
	Ok(page_container.data)
}

fn resolve_ptr_table_json(table: &[Value], index: usize) -> Result<Value> {
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

pub fn is_logged_in() -> bool {
	get_login_cookie().is_some_and(|_| {
		get_json_data::<UserProfile>("https://mangadot.net/api/profile")
			.is_ok_and(|user_profile| user_profile.profile.is_some_and(|p| p.id.is_some()))
	})
}

fn is_official_like(chapter: &MangaChapter) -> bool {
	let official_group_ids = [
		17423, // Official
		18142, // Animate International
		3521,  // Comikey
		5952,  // FAKKU
		3891,  // J-Novel Club
		9438,  // Kodansha USA
		10712, // Manga Plus
		18036, // Manga UP!
		18180, // One Peace Books
		18052, // Seven Seas Entertainment
		18234, // Square Enix Manga
		16861, // Viz Manga
		17842, // VIZ Media
		17841, // VIZ Shonen Jump
		13541, // Yen Press
		10887, // Manta
		16168, // Tapas
		16170, // TappyToon
		10110, // LINE Webtoon
		16424, // Toomics
	];

	// There are probably others but tbh, they have not standardized this properly so this is
	// only a small chunk that I know of. Wait for the site to mature better before optimizing
	// this function. (And this only works for maybe 1% of the manga available)
	let official_scanlator_names = ["Official", "Official?", "MangaPlus", "Comikey", "K-Manga"];

	let group_id = chapter
		.group_id
		.as_ref()
		.is_some_and(|id| official_group_ids.contains(id));

	let group_ids = chapter
		.groups
		.as_ref()
		.is_some_and(|groups| groups.iter().any(|g| official_group_ids.contains(&g.id)));

	let scanlator_name = chapter.scanlator_name.as_ref().is_some_and(|name| {
		official_scanlator_names
			.iter()
			.any(|s| s.to_lowercase() == name.to_lowercase())
	});

	group_id || group_ids || scanlator_name
}

fn find_personal_group_preference_index(chapter: &MangaChapter) -> Option<usize> {
	let deduped_group_list = get_deduped_group_list();
	let mut index: Vec<Option<usize>> = Vec::new();

	let group_id = chapter.group_id.as_ref().and_then(|id| {
		deduped_group_list
			.iter()
			.position(|p| p == &format!("{id}"))
	});
	index.push(group_id);

	if let Some(groups) = chapter.groups.as_ref() {
		groups.iter().for_each(|g| {
			index.push(
				deduped_group_list
					.iter()
					.position(|p| p == &format!("{}", g.id)),
			);
		});
	}

	index.into_iter().flatten().min()
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

	let order_new = find_personal_group_preference_index(new);
	let order_cur = find_personal_group_preference_index(current);

	if order_new.is_some() && order_cur.is_none() {
		return true;
	}

	if order_new.is_some() && order_cur.is_some() && order_new.lt(&order_cur) {
		return true;
	}

	let new_created_at = new.created_at();
	let cur_created_at = current.created_at();
	new_created_at > cur_created_at
}

pub fn dedup_insert(map: &mut HashMap<String, MangaChapter>, chapter: MangaChapter) {
	let key: String = chapter
		.chapter_number
		.map(|n| n.to_string())
		.unwrap_or("0".into());
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
