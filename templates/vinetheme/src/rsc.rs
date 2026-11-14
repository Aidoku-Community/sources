use aidoku::{
	Result,
	alloc::{String, Vec, borrow::Cow},
	prelude::*,
};
use serde::de::DeserializeOwned;
use serde_json::Value;

pub fn decode(body: &str) -> Result<Cow<'_, str>> {
	if !body.trim_start().starts_with('<') {
		return Ok(Cow::Borrowed(body));
	}
	let mut flight = String::new();
	for chunk in body.split("self.__next_f.push(").skip(1) {
		let Some(Ok(value)) = serde_json::Deserializer::from_str(chunk)
			.into_iter::<Value>()
			.next()
		else {
			continue;
		};
		if value[0] == 1
			&& let Some(text) = value[1].as_str()
		{
			flight.push_str(text);
		}
	}
	if flight.is_empty() {
		bail!("Missing page data");
	}
	Ok(Cow::Owned(flight))
}

pub fn extract<T: DeserializeOwned>(body: &str, keys: &[&str]) -> Result<T> {
	let value = objects(body, keys)
		.into_iter()
		.next()
		.ok_or(error!("Missing {} in page data", keys.join("/")))?;
	Ok(serde_json::from_value(value)?)
}

pub fn objects(body: &str, keys: &[&str]) -> Vec<Value> {
	let mut output = Vec::new();
	for (_, row) in rows(body) {
		if let Row::Json(line) = row
			&& let Ok(value) = serde_json::from_str(line)
		{
			collect(value, keys, &mut output);
		}
	}
	output
}

enum Row<'a> {
	Text(&'a str),
	Json(&'a str),
}

fn rows(body: &str) -> impl Iterator<Item = (&str, Row<'_>)> {
	let mut remaining = body;
	core::iter::from_fn(move || {
		loop {
			let (id, row) = remaining.split_once(':')?;
			if id.is_empty() || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
				remaining = remaining.split_once('\n').map_or("", |(_, rest)| rest);
				continue;
			}
			if let Some(raw) = row.strip_prefix('T') {
				let (length, text) = raw.split_once(',')?;
				let length = usize::from_str_radix(length, 16).ok()?;
				// Byte-length text rows may contain newlines or touch the next row.
				let value = text.get(..length)?;
				remaining = text.get(length..)?.trim_start_matches('\n');
				return Some((id, Row::Text(value)));
			}
			let (line, rest) = row.split_once('\n').unwrap_or((row, ""));
			remaining = rest;
			return Some((id, Row::Json(line)));
		}
	})
}

fn collect(value: Value, keys: &[&str], output: &mut Vec<Value>) {
	match value {
		Value::Object(object) if keys.iter().all(|key| object.contains_key(*key)) => {
			output.push(Value::Object(object))
		}
		Value::Object(object) => {
			for (_, value) in object {
				collect(value, keys, output);
			}
		}
		Value::Array(values) => {
			for value in values {
				collect(value, keys, output);
			}
		}
		_ => {}
	}
}

pub fn text<'a>(body: &'a str, value: &'a str) -> Result<Cow<'a, str>> {
	let Some(id) = value
		.strip_prefix('$')
		.filter(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_hexdigit()))
	else {
		return Ok(Cow::Borrowed(value));
	};
	let (_, row) = rows(body)
		.find(|(key, _)| *key == id)
		.ok_or(error!("Missing chapter text"))?;
	match row {
		Row::Text(text) => Ok(Cow::Borrowed(text)),
		Row::Json(json) => Ok(Cow::Owned(serde_json::from_str(json)?)),
	}
}
