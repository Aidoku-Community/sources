use crate::BASE_URL;
use aidoku::{
	Result,
	alloc::{String, collections::BTreeMap},
	imports::html::{Document, Element, Html},
	prelude::*,
};
use serde::de::DeserializeOwned;
use serde_json::Value;

pub fn chunks(doc: &Document) -> BTreeMap<String, Element> {
	doc.select("div[id^='S:']")
		.into_iter()
		.flatten()
		.filter_map(|el| Some((el.attr("id")?.strip_prefix("S:")?.into(), el)))
		.collect()
}

// React streams both whole entries and fragments inside an entry.
pub fn expand(row: &Element, chunks: &mut BTreeMap<String, Element>) -> Option<String> {
	// An empty chunk has no html at all.
	let mut html = row.html().unwrap_or_default();
	for slot in row.select("template[id^='P:']").into_iter().flatten() {
		let id = slot.attr("id")?;
		if let Some(chunk) = chunks.remove(id.strip_prefix("P:")?) {
			html = html.replacen(&slot.outer_html()?, &expand(&chunk, chunks)?, 1);
		}
	}
	Some(html)
}

pub fn chapter_list(doc: &Document) -> Result<Document> {
	let section = doc
		.select_first("section#chapters")
		.ok_or_else(|| error!("Webnex did not return a chapter section"))?;
	let html = expand(&section, &mut chunks(doc))
		.ok_or_else(|| error!("Webnex did not return the chapter list"))?;
	Ok(Html::parse_with_url(html, BASE_URL)?)
}

// Next.js pushes its page data as JSON strings, one `id:json` row per line.
pub fn extract<T: DeserializeOwned>(doc: &Document, keys: &[&str]) -> Option<T> {
	let mut flight = String::new();
	for script in doc.select("script").into_iter().flatten() {
		if let Some(data) = script.html()
			&& let Some(json) = data.trim().strip_prefix("self.__next_f.push([1,")
			&& let Some(Ok(chunk)) = serde_json::Deserializer::from_str(json)
				.into_iter::<String>()
				.next()
		{
			flight.push_str(&chunk);
		}
	}
	flight
		.lines()
		.filter_map(|line| serde_json::from_str(line.split_once(':')?.1).ok())
		.find_map(|value| find(value, keys))
		.and_then(|value| serde_json::from_value(value).ok())
}

fn find(value: Value, keys: &[&str]) -> Option<Value> {
	match value {
		Value::Object(object) if keys.iter().all(|key| object.contains_key(*key)) => {
			Some(Value::Object(object))
		}
		Value::Object(object) => object.into_iter().find_map(|(_, value)| find(value, keys)),
		Value::Array(values) => values.into_iter().find_map(|value| find(value, keys)),
		_ => None,
	}
}
