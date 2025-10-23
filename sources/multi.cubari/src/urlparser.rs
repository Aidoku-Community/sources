use aidoku::{
	alloc::{String, Vec, string::ToString},
	prelude::format,
};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

fn base64_encode<T: AsRef<[u8]>>(str: T) -> String {
	URL_SAFE_NO_PAD.encode(str.as_ref())
}

/// Convert a compatible URL to a Cubari slug.
///
/// Currently works with:
/// - Imgur, Reddit, imgbox, imgchest, catbox gallery URLs
/// - GitHub Gists raw URLs
/// - git.io URLs
/// - nhentai, weebcentral URLs
/// - cubari.moe reader page URLs
///
/// # Returns
/// Returns the original URL if not parsable.
pub fn url_to_slug<T: AsRef<str>>(url: T) -> String {
	let url = url.as_ref();
	let slash_count = url.matches('/').count();
	let query = url
		.trim_start_matches("Http")
		.trim_start_matches("http")
		.trim_start_matches('s')
		.trim_start_matches("://")
		.trim_end_matches('/');
	if query.contains("imgur") && query.replace("/a/", "/gallery/").contains("/gallery/")
		|| query.contains("reddit.com/gallery")
		|| query.contains("imgbox.com/g/")
		|| query.contains("nhentai.net/g/")
		|| query.contains("imgchest.com/p/")
		|| query.contains("catbox.moe/c/")
	{
		// Common parser for any URL with this structure
		// https://{source}.{tld}/path/{slug}
		// where slug is always the last part of the URL.
		let domain = query.split('/').next().unwrap_or_default();
		let source = domain.split('.').nth_back(1).unwrap_or_default();
		let slug = query.split('/').next_back().unwrap_or_default();
		format!("{source}/{slug}")
	} else if query.contains("git.io") {
		format!("gist/{}", query.trim_start_matches("git.io/"))
	} else if query.contains("gist.githubusercontent.com/")
		|| query.contains("gist.github.com/") && query.contains("raw")
	{
		let temp = format!(
			"gist/{}",
			query
				.trim_start_matches("gist.githubusercontent.com/")
				.trim_start_matches("gist.github.com/"),
		);
		format!("gist/{}", base64_encode(temp))
	} else if query.contains("weebcentral.com/series") {
		let split = query.split('/').collect::<Vec<_>>();
		format!(
			"weebcentral/{}",
			split.get(2).map(|s| s.to_string()).unwrap_or_default()
		)
	} else if query.contains("mangadex.org/title") {
		let split = query.split('/').collect::<Vec<_>>();
		format!("mangadex/{}", split[2])
	} else if query.contains("cubari.moe/read") && slash_count >= 3 {
		let split = query
			.trim_start_matches("cubari.moe/read/")
			.trim_end_matches('/')
			.split('/')
			.collect::<Vec<_>>();
		format!("{}/{}", split[0], split[1])
	} else if slash_count == 1 || (slash_count == 2 && url.ends_with('/')) {
		// normalize input slugs
		let fragments = query.split('/').collect::<Vec<_>>();
		format!("{}/{}", fragments[0].to_lowercase(), fragments[1])
	} else {
		url.into()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	macro_rules! generate_test {
		($url:expr, $matches:expr) => {
			assert_eq!(url_to_slug($url), $matches);
			assert_eq!(url_to_slug("http://".to_string() + $url), $matches);
			assert_eq!(url_to_slug("https://".to_string() + $url), $matches);
			assert_eq!(url_to_slug("http://".to_string() + $url + "/"), $matches);
			assert_eq!(url_to_slug("https://".to_string() + $url + "/"), $matches);
		};
	}

	#[aidoku_test]
	fn test_source_slug_parser() {
		generate_test!("reddit.com/gallery/vjry2h", "reddit/vjry2h");
		generate_test!("new.reddit.com/gallery/vjry2h", "reddit/vjry2h");
		generate_test!("www.reddit.com/gallery/vjry2h", "reddit/vjry2h");

		generate_test!("imgur.com/gallery/hYhqG7b", "imgur/hYhqG7b");
		generate_test!("imgur.io/gallery/hYhqG7b", "imgur/hYhqG7b");
		generate_test!("m.imgur.com/gallery/hYhqG7b", "imgur/hYhqG7b");

		generate_test!("imgur.com/a/hYhqG7b", "imgur/hYhqG7b");
		generate_test!("imgur.io/a/hYhqG7b", "imgur/hYhqG7b");
		generate_test!("m.imgur.com/a/hYhqG7b", "imgur/hYhqG7b");

		generate_test!("nhentai.net/g/177013", "nhentai/177013");
		generate_test!("imgbox.com/g/YMWC88hgjM", "imgbox/YMWC88hgjM");
		generate_test!("imgchest.com/p/wl7lorjzx7x", "imgchest/wl7lorjzx7x");
		generate_test!("catbox.moe/c/zj34br", "catbox/zj34br");
	}

	#[aidoku_test]
	fn test_git_io_parser() {
		generate_test!("git.io/JO7JN", "gist/JO7JN");
	}

	#[aidoku_test]
	fn test_github_gist_parser() {
		generate_test!(
			"gist.github.com/NightA/99cf38923b5b80d62b83158c141a1226/raw/9eed3fad738ed66943804cbb27df5404d5586b07/Yofukashi.JSON",
			"gist/Z2lzdC9OaWdodEEvOTljZjM4OTIzYjViODBkNjJiODMxNThjMTQxYTEyMjYvcmF3LzllZWQzZmFkNzM4ZWQ2Njk0MzgwNGNiYjI3ZGY1NDA0ZDU1ODZiMDcvWW9mdWthc2hpLkpTT04"
		);
		generate_test!(
			"gist.githubusercontent.com/NightA/99cf38923b5b80d62b83158c141a1226/raw/9eed3fad738ed66943804cbb27df5404d5586b07/Yofukashi.JSON",
			"gist/Z2lzdC9OaWdodEEvOTljZjM4OTIzYjViODBkNjJiODMxNThjMTQxYTEyMjYvcmF3LzllZWQzZmFkNzM4ZWQ2Njk0MzgwNGNiYjI3ZGY1NDA0ZDU1ODZiMDcvWW9mdWthc2hpLkpTT04"
		);
	}

	#[aidoku_test]
	fn test_weebcentral_parser() {
		generate_test!(
			"weebcentral.com/series/01J76XYC7KP5M48JAY8S4ZNDX5/Kanojo-Okarishimasu/",
			"weebcentral/01J76XYC7KP5M48JAY8S4ZNDX5"
		);
		generate_test!(
			"weebcentral.com/series/01J76XYC7KP5M48JAY8S4ZNDX5",
			"weebcentral/01J76XYC7KP5M48JAY8S4ZNDX5"
		);
	}

	#[aidoku_test]
	fn test_mangadex_parser() {
		generate_test!(
			"mangadex.org/title/801513ba-a712-498c-8f57-cae55b38cc92/berserk",
			"mangadex/801513ba-a712-498c-8f57-cae55b38cc92"
		);
		generate_test!(
			"mangadex.org/title/801513ba-a712-498c-8f57-cae55b38cc92",
			"mangadex/801513ba-a712-498c-8f57-cae55b38cc92"
		);
	}

	#[aidoku_test]
	fn test_cubari_parser() {
		generate_test!(
			"cubari.moe/read/gist/Z2lzdC9OaWdodEEvOTljZjM4OTIzYjViODBkNjJiODMxNThjMTQxYTEyMjYvcmF3LzllZWQzZmFkNzM4ZWQ2Njk0MzgwNGNiYjI3ZGY1NDA0ZDU1ODZiMDcvWW9mdWthc2hpLkpTT04",
			"gist/Z2lzdC9OaWdodEEvOTljZjM4OTIzYjViODBkNjJiODMxNThjMTQxYTEyMjYvcmF3LzllZWQzZmFkNzM4ZWQ2Njk0MzgwNGNiYjI3ZGY1NDA0ZDU1ODZiMDcvWW9mdWthc2hpLkpTT04"
		);
		generate_test!("cubari.moe/read/nhentai/408179", "nhentai/408179");
		generate_test!("cubari.moe/read/nhentai/408179/1", "nhentai/408179");
	}

	#[aidoku_test]
	fn test_normalization() {
		assert_eq!(
			url_to_slug("Weebcentral/01J76XYC7KP5M48JAY8S4ZNDX5"),
			"weebcentral/01J76XYC7KP5M48JAY8S4ZNDX5"
		);
		assert_eq!(
			url_to_slug("weebcentral/01J76XYC7KP5M48JAY8S4ZNDX5/"),
			"weebcentral/01J76XYC7KP5M48JAY8S4ZNDX5"
		);
		assert_eq!(
			url_to_slug("Weebcentral/01J76XYC7KP5M48JAY8S4ZNDX5/"),
			"weebcentral/01J76XYC7KP5M48JAY8S4ZNDX5"
		);
	}

	#[aidoku_test]
	fn test_unknown_parser() {
		assert_eq!(
			url_to_slug("https://www.google.com"),
			"https://www.google.com"
		);
		assert_eq!(url_to_slug("nhentai/177013"), "nhentai/177013");
	}
}
