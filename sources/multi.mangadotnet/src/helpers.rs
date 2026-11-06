use crate::settings::get_cloudflare_cookie;
use crate::{
	BASE_URL, CLOUDFLARE_COOKIE_KEY, LOGIN_COOKIE_KEY, models::MangaChapter, models::PageContainer,
	models::UserProfile, settings::get_deduped_group_list, settings::get_login_cookie,
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

const CF_CHALLENGE_ERROR_MESSAGE: &str = "Response returned CF challenge page instead of JSON data. If problem persist, please clear the source cache and use the Resolve Cloudflare button instead.";

fn create_request_get(url: &str) -> Result<Request> {
	let mut request = Request::get(url)?;
	let mut cookies = Vec::<String>::new();

	if let Some(token) = get_login_cookie() {
		cookies.push(format!("{LOGIN_COOKIE_KEY}={token}"));
	}

	if let Some(token) = get_cloudflare_cookie() {
		cookies.push(format!("{CLOUDFLARE_COOKIE_KEY}={token}"));
	}

	if !cookies.is_empty() {
		request = request.header("Cookie", &cookies.join(";"));
	}

	Ok(request)
}

fn response_is_ok(response: &Response) -> Result<()> {
	if response
		.get_header("cf-mitigated")
		.is_some_and(|value| value == "challenge")
	{
		bail!("{CF_CHALLENGE_ERROR_MESSAGE}")
	} else if response.status_code() == 503 {
		bail!("Website is under maintenance. Please try again later :)");
	} else if response.status_code() == 429 {
		if let Some(duration) = response.get_header("Retry-After") {
			bail!("Rate limited. Please wait for {duration} seconds before trying again.");
		} else {
			bail!("Rate limited. Please try again later.");
		}
	} else if response.status_code() == 401 {
		bail!("Login token expired. Please re-login in the source settings.");
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

/*
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
*/

fn handle_page_container_json_data_response<T>(response: &Response) -> Result<T>
where
	T: DeserializeOwned,
{
	let ptr_table_json = serde_json::from_slice::<Vec<Value>>(&response.get_data()?)?;
	let json = resolve_ptr_table_json(&ptr_table_json, 0)?;
	let page_container_json = serde_json::from_value::<HashMap<String, PageContainer<T>>>(json)?;
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

pub fn base_url_join(uri: &str) -> String {
	if uri.starts_with("/") {
		format!("{BASE_URL}{uri}")
	} else {
		format!("{BASE_URL}/{uri}")
	}
}

fn normalize_name(name: &str) -> String {
	name.to_lowercase()
}

fn get_chapter_priority(
	chapter: &MangaChapter,
	official_groups_names: &[&str],
	deduped_group_list: &[String],
	group_frequency_map: &HashMap<String, u32>,
) -> (u32, usize, u32, i64) {
	let mut combined_group: Vec<String> = Vec::new();

	if let Some(groups) = chapter.groups.as_ref() {
		groups.iter().for_each(|g| {
			combined_group.push(normalize_name(&g.name));
			combined_group.push(g.id.to_string())
		})
	}

	if let Some(scanlator) = chapter.scanlator_name.as_ref() {
		combined_group.push(normalize_name(scanlator));
	}

	let frequency = combined_group
		.iter()
		.map(|name| group_frequency_map.get(name).copied().unwrap_or(0))
		.max()
		.unwrap_or(0);

	let created_at = chapter.created_at().unwrap_or(0);

	// Rule 1: Official group name, then frequency, then upload datetime.
	if combined_group
		.iter()
		.any(|g| official_groups_names.iter().any(|official| g.eq(official)))
	{
		return (0, 0, frequency, created_at);
	}

	// Rule 2: Group name/id in deduped_group_list.
	let best_position = combined_group
		.iter()
		.filter_map(|group| {
			deduped_group_list
				.iter()
				.position(|name| normalize_name(name) == *group)
		})
		.min();

	if let Some(position) = best_position {
		return (1, position, frequency, created_at);
	}

	// Rule 3: Most frequent group name / scanlator name.
	(2, 0, frequency, created_at)
}

fn deduplicate_chapters(
	chapters: Vec<MangaChapter>,
	official_groups_names: &[&str],
	deduped_group_list: &[String],
	group_frequency_map: &HashMap<String, u32>,
) -> Vec<MangaChapter> {
	// Calculate priorities and retain original indices for stable tie-breaking.
	let mut ranked: Vec<_> = chapters
		.into_iter()
		.enumerate()
		.map(|(index, chapter)| {
			let priority = get_chapter_priority(
				&chapter,
				official_groups_names,
				deduped_group_list,
				group_frequency_map,
			);

			(index, priority, chapter)
		})
		.collect();

	// Higher priority first.
	ranked.sort_by(|a, b| {
		a.1.0
			.cmp(&b.1.0) // Ruleset
			.then_with(|| a.1.1.cmp(&b.1.1)) // Position
			.then_with(|| b.1.2.cmp(&a.1.2)) // Frequency
			.then_with(|| b.1.3.cmp(&a.1.3)) // Datetime
			.then_with(|| a.0.cmp(&b.0))
	});

	let mut seen = Vec::new();
	let mut result = Vec::new();

	for (_, _, chapter) in ranked {
		if let Some(number) = chapter.chapter_number {
			if seen.contains(&number) {
				continue;
			} else {
				seen.push(number);
			}
		}

		result.push(chapter);
	}

	result
}

pub fn build_dedupe_chapters(input: Vec<MangaChapter>) -> Vec<MangaChapter> {
	let official_groups_names = [
		"official",
		"official?",
		"animate international",
		"comikey",
		"dark horse manga",
		"fakku",
		"j-novel club",
		"k-manga",
		"k manga",
		"kodansha usa",
		"kodansha comics",
		"manga plus",
		"mangaplus",
		"manga up!",
		"one peace books",
		"seven seas entertainment",
		"square enix manga",
		"viz manga",
		"viz media",
		"viz shonen jump",
		"yen press",
		"manta",
		"tapas",
		"tappytoon",
		"line webtoon",
		"toomics",
		"webtoon",
	];

	let mut group_frequency_map: HashMap<String, u32> = HashMap::new();
	let mut chapter_map: HashMap<u32, Vec<String>> = HashMap::new();

	fn increment_group_frequency(map: &mut HashMap<String, u32>, group: &str) {
		if let Some(frequency) = map.get_mut(group) {
			*frequency += 1;
		} else {
			map.insert(group.into(), 1);
		}
	}

	for chapter in input.iter() {
		let chapter_number = chapter.chapter_number.map(|n| n as u32).unwrap_or(0);

		if let Some(group_map) = chapter_map.get_mut(&chapter_number) {
			if let Some(groups) = chapter.groups.as_ref() {
				groups.iter().for_each(|g| {
					let group_name = normalize_name(&g.name);

					if !group_map.contains(&group_name) {
						increment_group_frequency(&mut group_frequency_map, &group_name);
						group_map.push(group_name);
					}
				})
			}

			if let Some(scanlator) = chapter.scanlator_name.as_ref() {
				let normalized_scanlator = normalize_name(scanlator);

				if !group_map.contains(&normalized_scanlator) {
					increment_group_frequency(&mut group_frequency_map, &normalized_scanlator);
					group_map.push(normalized_scanlator);
				}
			}
		} else {
			let mut value: Vec<String> = Vec::new();

			if let Some(groups) = chapter.groups.as_ref() {
				groups.iter().for_each(|g| {
					let group_name = normalize_name(&g.name);
					increment_group_frequency(&mut group_frequency_map, &group_name);
					value.push(group_name);
				})
			}

			if let Some(scanlator) = chapter.scanlator_name.as_ref() {
				let normalized_scanlator = normalize_name(scanlator);
				increment_group_frequency(&mut group_frequency_map, &normalized_scanlator);
				value.push(normalized_scanlator);
			}

			chapter_map.insert(chapter_number, value);
		}
	}

	deduplicate_chapters(
		input,
		&official_groups_names,
		&get_deduped_group_list(),
		&group_frequency_map,
	)
}
