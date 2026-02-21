use aidoku::alloc::string::String;

/// Strip trailing "Raw Free" and similar suffixes from a title.
pub fn clean_title(title: String) -> String {
	let suffixes = [" Raw Free", " Raw free", " raw free"];
	for suffix in suffixes {
		if let Some(clean) = title.strip_suffix(suffix) {
			return clean.trim().into();
		}
	}
	title
}

/// Extract chapter number from text like 【第N話】 -> N
pub fn extract_ch_number(s: &str) -> Option<f32> {
	let dai = '第';
	let wa = '話';

	let start = s.find(dai)? + dai.len_utf8();
	let end = s[start..].find(wa)? + start;

	let num_str = &s[start..end];
	num_str.parse().ok()
}
