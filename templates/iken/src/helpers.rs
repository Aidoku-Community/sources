use aidoku::alloc::String;

// for eternalmangas.com
fn remove_accents(c: char) -> Option<char> {
	match c {
		'á' | 'à' | 'ä' | 'â' => Some('a'),
		'é' | 'è' | 'ë' | 'ê' => Some('e'),
		'í' | 'ì' | 'ï' | 'î' => Some('i'),
		'ó' | 'ò' | 'ö' | 'ô' => Some('o'),
		'ú' | 'ù' | 'ü' | 'û' => Some('u'),
		'ñ' => Some('n'),
		'ç' => Some('c'),
		_ => None,
	}
}

pub fn slugify(input: &str) -> String {
	let mut slug = String::new();
	let mut prev_hyphen = false;

	for c in input.chars() {
		let c = c.to_ascii_lowercase();
		let c = remove_accents(c).unwrap_or(c);

		if c.is_ascii_alphanumeric() || c == '\'' {
			slug.push(c);
			prev_hyphen = false;
		} else if c.is_whitespace() || c == '-' {
			#[allow(clippy::collapsible_if)]
			if !prev_hyphen && !slug.is_empty() {
				slug.push('-');
				prev_hyphen = true;
			}
		}
	}

	// remove trailing hyphen if present
	if slug.ends_with('-') {
		slug.pop();
	}

	slug
}
