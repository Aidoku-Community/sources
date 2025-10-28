use aidoku::{
	alloc::{format, string::String, vec::Vec},
	imports::html::Element,
};

pub trait ElementImageAttr {
	fn img_attr(&self) -> Option<String>;
}

impl ElementImageAttr for Element {
	fn img_attr(&self) -> Option<String> {
		self.attr("abs:data-lazy-src")
			.or_else(|| self.attr("abs:data-src"))
			.or_else(|| self.attr("abs:src"))
	}
}

pub fn extract_images(content: &str) -> Vec<String> {
	let slice = format!(
		"[{}]",
		extract_between(content, "\"images\":[", "]").unwrap_or_default()
	);
	serde_json::from_str::<Vec<String>>(&slice).unwrap_or_default()
}

pub fn extract_between<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
	s.find(start).and_then(|start_idx| {
		let after_start = &s[start_idx + start.len()..];
		after_start.find(end).map(|end_idx| &after_start[..end_idx])
	})
}

pub fn selector(template: &str, values: &[&str]) -> String {
	let mut selectors = Vec::new();
	for value in values {
		// replace all occurrences of {} with the value
		let mut formatted = String::new();
		let mut parts = template.split("{}");
		if let Some(first) = parts.next() {
			formatted.push_str(first);
		}
		for part in parts {
			formatted.push_str(value);
			formatted.push_str(part);
		}
		selectors.push(formatted);
	}
	selectors.join(", ")
}

pub fn find_first_f32(s: &str) -> Option<f32> {
	let mut num = String::new();
	let mut found_digit = false;
	let mut dot_found = false;

	for c in s.chars() {
		if c.is_ascii_digit() {
			num.push(c);
			found_digit = true;
		} else if c == '.' && found_digit && !dot_found {
			num.push(c);
			dot_found = true;
		} else if found_digit {
			break;
		}
	}

	if found_digit {
		num.parse::<f32>().ok()
	} else {
		None
	}
}
