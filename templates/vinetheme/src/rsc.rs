use aidoku::{Result, prelude::*};
use serde::de::DeserializeOwned;
use serde_json::Value;

pub fn extract<T: DeserializeOwned>(body: &str, required_key: &str) -> Result<T> {
	for line in body.lines() {
		let candidate = line.split_once(':').map_or(line, |(_, value)| value);
		if let Ok(value) = serde_json::from_str::<Value>(candidate)
			&& let Some(found) = find_object(value, required_key)
			&& let Ok(result) = serde_json::from_value(found)
		{
			return Ok(result);
		}
	}

	if let Ok(value) = serde_json::from_str::<Value>(body)
		&& let Some(found) = find_object(value, required_key)
	{
		return Ok(serde_json::from_value(found)?);
	}

	bail!("Unable to find {required_key} in RSC response")
}

fn find_object(value: Value, required_key: &str) -> Option<Value> {
	match value {
		Value::Object(mut object) => {
			if object.contains_key(required_key) {
				return Some(Value::Object(object));
			}
			object
				.values_mut()
				.find_map(|value| find_object(core::mem::take(value), required_key))
		}
		Value::Array(values) => values
			.into_iter()
			.find_map(|value| find_object(value, required_key)),
		Value::String(string) => serde_json::from_str::<Value>(&string)
			.ok()
			.and_then(|value| find_object(value, required_key)),
		_ => None,
	}
}

#[cfg(test)]
mod tests {
	use super::extract;
	use aidoku::alloc::string::String;
	use aidoku_test::aidoku_test;
	use serde::Deserialize;

	#[derive(Deserialize)]
	struct Payload {
		series: Series,
	}

	#[derive(Deserialize)]
	struct Series {
		title: String,
	}

	#[aidoku_test]
	fn test_extraction() {
		let payload: Payload = extract(r#"1:["$",{"series":{"title":"Test"}}]"#, "series").unwrap();
		assert_eq!(payload.series.title, "Test");
	}
}
