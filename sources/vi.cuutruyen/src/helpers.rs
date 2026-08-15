use aidoku::alloc::string::{String, ToString};

const REPLACEMENTS: &[(&str, &str)] = &[
	("storage-ct.lrclib.net", "storage-bravo.cuutruyen.net"),
	("storage-ct-riften.site", "storage-charlie.cuutruyen.net"),
];

pub fn rewrite_storage_url(url: impl AsRef<str>) -> String {
	let mut result = url.as_ref().to_string();
	for &(old_host, new_host) in REPLACEMENTS {
		result = result.replace(old_host, new_host);
	}
	result
}
