use aidoku::{
	Result,
	alloc::{string::String, vec::Vec},
	imports::js::JsContext,
	prelude::*,
};

// var imgsrcs\s*=\s*['"]([a-zA-Z0-9+=/]+)['"]
pub fn extract_imgsrcs(input: &str) -> Option<&str> {
	let search = "var imgsrcs";
	let mut idx = input.find(search)?;
	idx += search.len();

	let bytes = input.as_bytes();
	let mut i = idx;
	while i < bytes.len() {
		let c = bytes[i] as char;
		if c.is_whitespace() || c == '=' {
			i += 1;
		} else {
			break;
		}
	}

	if i >= bytes.len() {
		return None;
	}
	let quote = bytes[i] as char;
	if quote != '\'' && quote != '"' {
		return None;
	}
	i += 1;
	let start = i;

	while i < bytes.len() {
		if bytes[i] as char == quote {
			return Some(&input[start..i]);
		}
		i += 1;
	}
	None
}

// https://github.com/keiyoushi/extensions-source/blob/ffb7e9f39230869cf4b98ab25b480a7b0a510bc0/src/en/mangago/src/eu/kanade/tachiyomi/extension/en/mangago/SoJsonV4Deobfuscator.kt
pub fn sojson_v4_decode(jsf: &str) -> Result<String> {
	if !jsf.starts_with("['sojson.v4']") {
		bail!("Obfuscated code is not sojson.v4");
	}

	if jsf.len() < 240 + 59 {
		bail!("Input too short");
	}
	let args_str = &jsf[240..jsf.len() - 59];

	let mut args = Vec::new();
	let mut num_start = None;
	for (i, c) in args_str.char_indices() {
		if c.is_ascii_alphabetic() {
			if let Some(start) = num_start {
				if start < i {
					args.push(&args_str[start..i]);
				}
				num_start = None;
			}
		} else if num_start.is_none() {
			num_start = Some(i);
		}
	}
	if let Some(start) = num_start
		&& start < args_str.len()
	{
		args.push(&args_str[start..]);
	}

	let mut result = String::new();
	for num_str in args {
		if num_str.is_empty() {
			continue;
		}
		let code = num_str.parse::<u32>().map_err(|_| error!("Parse error"))?;
		if let Some(ch) = core::char::from_u32(code) {
			result.push(ch);
		} else {
			bail!("Invalid char code");
		}
	}
	Ok(result)
}

// var <variable>\s*=\s*CryptoJS\.enc\.Hex\.parse\("(.*)"\)
pub fn find_hex_encoded_variable<'a>(input: &'a str, variable: &str) -> Option<&'a str> {
	let prefix = format!("var {variable}");
	let mut idx = input.find(&prefix)?;
	idx += prefix.len();

	let bytes = input.as_bytes();
	let mut i = idx;
	while i < bytes.len() {
		let c = bytes[i] as char;
		if c.is_whitespace() || c == '=' {
			i += 1;
		} else {
			break;
		}
	}

	let parse_str = "CryptoJS.enc.Hex.parse(";
	if !input[i..].starts_with(parse_str) {
		return None;
	}
	i += parse_str.len();

	if i >= bytes.len() {
		return None;
	}
	let quote = bytes[i] as char;
	if quote != '"' {
		return None;
	}
	i += 1;
	let start = i;

	while i < bytes.len() {
		if bytes[i] as char == '"' {
			return Some(&input[start..i]);
		}
		i += 1;
	}
	None
}

pub fn decode_hex(s: &str) -> Result<Vec<u8>> {
	let len = s.len();
	if !len.is_multiple_of(2) {
		bail!("Must have an even length");
	}

	let mut bytes = Vec::with_capacity(len / 2);
	let chars: Vec<_> = s.chars().collect();

	for i in (0..len).step_by(2) {
		let hi = chars[i]
			.to_digit(16)
			.ok_or_else(|| error!("Invalid hex digit"))?;
		let lo = chars[i + 1]
			.to_digit(16)
			.ok_or_else(|| error!("Invalid hex digit"))?;
		bytes.push(((hi << 4) | lo) as u8);
	}

	Ok(bytes)
}

// str\.charAt\(\s*(\d+)\s*\)
fn find_key_locations(js: &str) -> Vec<usize> {
	let mut locations = Vec::new();
	let mut i = 0;
	let pat = "str.charAt(";
	while let Some(start) = js[i..].find(pat) {
		let idx = i + start + pat.len();
		let rest = &js[idx..];
		let bytes = rest.as_bytes();
		let num_start = rest.find(|c: char| c.is_ascii_digit());
		if let Some(num_start) = num_start {
			let mut num_end = num_start;
			while num_end < rest.len() && bytes[num_end].is_ascii_digit() {
				num_end += 1;
			}
			let num_str = &rest[num_start..num_end];
			if let Ok(num) = num_str.parse::<usize>()
				&& !locations.contains(&num)
			{
				locations.push(num);
			}
			i = idx + num_end;
		} else {
			break;
		}
	}
	locations
}

fn unscramble(s: &mut [char], keys: &[usize]) {
	for &key in keys.iter().rev() {
		let len = s.len();
		for i in (key..len).rev() {
			if i % 2 != 0 {
				s.swap(i - key, i);
			}
		}
	}
}

pub fn unscramble_image_list(image_list: &str, js: &str) -> String {
	let mut img_list: Vec<char> = image_list.chars().collect();
	let key_locations = find_key_locations(js);

	let mut unscramble_key = Vec::new();
	for &loc in &key_locations {
		if let Some(digit) = img_list.get(loc).and_then(|c| c.to_digit(10)) {
			unscramble_key.push(digit as usize);
		} else {
			return image_list.into();
		}
	}

	for (idx, &loc) in key_locations.iter().enumerate() {
		let remove_at = loc - idx;
		if remove_at < img_list.len() {
			img_list.remove(remove_at);
		}
	}

	unscramble(&mut img_list, &unscramble_key);
	img_list.into_iter().collect()
}

// var widthnum\s*=\s*heightnum\s*=\s*(\d+);
pub fn find_cols(input: &str) -> Option<&str> {
	let pat = "var widthnum";
	let mut idx = input.find(pat)?;
	idx += pat.len();

	let bytes = input.as_bytes();
	let mut i = idx;
	while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b'=') {
		i += 1;
	}

	let height_pat = "heightnum";
	if !input[i..].starts_with(height_pat) {
		return None;
	}
	i += height_pat.len();

	while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b'=') {
		i += 1;
	}

	let start = i;
	while i < bytes.len() && bytes[i].is_ascii_digit() {
		i += 1;
	}
	if start == i {
		return None;
	}
	Some(&input[start..i])
}

const REPLACE_POS_JS: &str = "
function replacePos(strObj, pos, replacetext) {
    var str = strObj.substr(0, pos) + replacetext + strObj.substring(pos + 1, strObj.length);
    return str;
}
";
const JS_FILTERS: &[&str] = &[
	"jQuery",
	"document",
	"getContext",
	"toDataURL",
	"getImageData",
	"width",
	"height",
];

pub fn get_descrambling_key(deobf_chapter_js: &str, image_url: &str) -> Result<String> {
	let after = match deobf_chapter_js.split_once("var renImg = function(img,width,height,id){") {
		Some((_, after)) => after,
		None => bail!("Pattern not found"),
	};
	let before = match after.split_once("key = key.split(") {
		Some((before, _)) => before,
		None => bail!("Pattern not found"),
	};

	let imgkeys: String = before
		.lines()
		.filter(|line| JS_FILTERS.iter().all(|f| !line.contains(f)))
		.collect::<Vec<_>>()
		.join("\n")
		.replace("img.src", "url");

	let js = format!(
		"function getDescramblingKey(url) {{ {}; return key; }}\ngetDescramblingKey(\"{}\");",
		imgkeys, image_url
	);

	let context = JsContext::new();
	context.eval(REPLACE_POS_JS)?;
	let result = context.eval(&js)?;

	Ok(result)
}

pub fn parse_chapter_title(input: &str) -> (Option<f32>, Option<f32>, Option<String>) {
	let mut volume = None;
	let mut chapter = None;
	let mut title = None;

	let trimmed = input.trim();

	let (left, right) = match trimmed.find(':') {
		Some(idx) => (&trimmed[..idx], Some(trimmed[idx + 1..].trim())),
		None => (trimmed, None),
	};

	let mut left = left.trim();
	if left.starts_with("Vol.") {
		left = left[4..].trim_start();
		let bytes = left.as_bytes();
		let mut vol_end = 0;
		while vol_end < left.len()
			&& ((bytes[vol_end] as char).is_ascii_digit() || (bytes[vol_end] as char) == '.')
		{
			vol_end += 1;
		}
		if vol_end > 0
			&& let Ok(v) = left[..vol_end].parse::<f32>()
		{
			volume = Some(v);
		}
		left = left[vol_end..].trim_start();
	}

	if left.starts_with("Ch.") {
		left = left[3..].trim_start();
		let bytes = left.as_bytes();
		let mut ch_end = 0;
		while ch_end < left.len()
			&& ((bytes[ch_end] as char).is_ascii_digit() || (bytes[ch_end] as char) == '.')
		{
			ch_end += 1;
		}
		if ch_end > 0
			&& let Ok(c) = left[..ch_end].parse::<f32>()
		{
			chapter = Some(c);
		}
		left = left[ch_end..].trim_start();
	}

	if let Some(t) = right {
		if !t.is_empty() {
			title = Some(t.into());
		}
	} else if !left.is_empty() {
		title = Some(left.into());
	}

	(volume, chapter, title)
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn test_parse_chapter_title() {
		assert_eq!(
			parse_chapter_title("Vol.7 Ch.38 : Title"),
			(Some(7.0), Some(38.0), Some("Title".into()))
		);
		assert_eq!(
			parse_chapter_title("Ch.32.5 : Title"),
			(None, Some(32.5), Some("Title".into()))
		);
		assert_eq!(parse_chapter_title("Vol.2"), (Some(2.0), None, None));
		assert_eq!(
			parse_chapter_title("Title"),
			(None, None, Some("Title".into()))
		);
		assert_eq!(
			parse_chapter_title("Vol.3 Ch.12.5"),
			(Some(3.0), Some(12.5), None)
		);
		assert_eq!(parse_chapter_title("Ch.1"), (None, Some(1.0), None));
	}
}
