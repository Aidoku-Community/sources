use aidoku::alloc::string::String;

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
