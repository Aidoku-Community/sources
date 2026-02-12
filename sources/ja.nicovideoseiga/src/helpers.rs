use aidoku::alloc::string::{String, ToString};

pub fn parse_chapter_number(s: &str) -> Option<(f32, String)> {
	let rest = s.strip_prefix('第')?;

	// find the end of the chapter number: first non-digit, non-dash
	let mut end = None;
	for (i, c) in rest.char_indices() {
		let is_ascii_digit = c.is_ascii_digit();
		let is_fullwidth_digit = ('０'..='９').contains(&c);
		let is_dash = c == '-';
		if !(is_ascii_digit || is_fullwidth_digit || is_dash) {
			end = Some(i);
			break;
		}
	}
	let end = end.unwrap_or(rest.len());
	let num_str = &rest[..end];

	let ascii_num: String = num_str
		.chars()
		.filter_map(|c| match c {
			'0'..='9' => Some(c),
			'０'..='９' => Some(((c as u32) - 0xFF10 + 0x30) as u8 as char),
			'-' => Some('.'),
			_ => None,
		})
		.collect();

	if ascii_num.is_empty() {
		None
	} else {
		let chapter = ascii_num.parse::<f32>().ok()?;
		// remove the "第...話" or "第... " part, and any following whitespace
		let mut after_marker = &rest[end..];
		if after_marker.starts_with('話') {
			after_marker = &after_marker['話'.len_utf8()..];
			after_marker = after_marker.trim_start();
		} else if after_marker
			.chars()
			.next()
			.is_some_and(|c| c.is_whitespace())
		{
			after_marker = after_marker.trim_start();
		}
		let cleaned_title = after_marker.to_string();
		Some((chapter, cleaned_title))
	}
}

#[cfg(test)]
mod tests {
	use super::parse_chapter_number;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn test_arabic_with_wa() {
		let s = "第5話 恥ずかしい戦いが始まった";
		assert_eq!(
			parse_chapter_number(s),
			Some((5.0, "恥ずかしい戦いが始まった".into()))
		);
	}

	#[aidoku_test]
	fn test_fullwidth_with_wa() {
		let s = "第５話　恥ずかしい戦いが始まった";
		assert_eq!(
			parse_chapter_number(s),
			Some((5.0, "恥ずかしい戦いが始まった".into()))
		);
	}

	#[aidoku_test]
	fn test_arabic_with_space() {
		let s = "第12 テスト";
		assert_eq!(parse_chapter_number(s), Some((12.0, "テスト".into())));
	}

	#[aidoku_test]
	fn test_arabic_no_space() {
		let s = "第1うろろ「かーきーかーくー」";
		assert_eq!(
			parse_chapter_number(s),
			Some((1.0, "うろろ「かーきーかーくー」".into()))
		);
	}

	#[aidoku_test]
	fn test_fullwidth_no_space() {
		let s = "第１２テスト";
		assert_eq!(parse_chapter_number(s), Some((12.0, "テスト".into())));
	}

	#[aidoku_test]
	fn test_no_marker() {
		let s = "【お気に入り10万突破記念】描きおろしイラスト公開！";
		assert_eq!(parse_chapter_number(s), None);
	}
}
