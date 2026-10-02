use aidoku::alloc::string::String;

pub fn clean_title(title: String) -> String {
	let suffixes = ["(Raw – Free)", "(Raw - Free)"];
	for suffix in suffixes {
		if let Some(clean) = title.strip_suffix(suffix) {
			return clean.into();
		}
	}
	title
}
