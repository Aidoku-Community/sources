/// Extracts the first quoted base64 substring starting with "WyJodHRw" (`["http`).
pub fn extract_page_base64(haystack: &str) -> Option<&str> {
	let bytes = haystack.as_bytes();
	let needle = b"WyJodHRw";
	let len = bytes.len();

	let mut i = 0;
	while i < len {
		// find starting quote
		let quote = match bytes[i] {
			b'\'' | b'"' => bytes[i],
			_ => {
				i += 1;
				continue;
			}
		};

		// check starting prefix
		let start = i + 1;
		let end = start + needle.len();
		if end > len || &bytes[start..end] != needle {
			i += 1;
			continue;
		}

		// ensure content is base64
		let mut j = end;
		while j < len {
			let c = bytes[j];
			// base64: [A-Za-z0-9_+/=]
			let is_base64 =
				c.is_ascii_alphanumeric() || c == b'_' || c == b'+' || c == b'/' || c == b'=';
			if !is_base64 {
				break;
			}
			j += 1;
		}

		// find ending quote
		if j < len && bytes[j] == quote {
			let s = &haystack[start..j]; // slice excluding quotes
			return Some(s);
		}

		i += 1;
	}
	None
}
