use super::*;
use aidoku_test::aidoku_test;

const TEST_BASE_URL: &str = "https://api.cdnlibs.org";

#[aidoku_test]
fn basic_urls() {
	assert_eq!(
		Url::manga_search(TEST_BASE_URL),
		"https://api.cdnlibs.org/api/manga"
	);

	assert_eq!(
		Url::manga_details(TEST_BASE_URL, "test-manga"),
		"https://api.cdnlibs.org/api/manga/test-manga"
	);

	assert_eq!(
		Url::manga_chapters(TEST_BASE_URL, "test-manga"),
		"https://api.cdnlibs.org/api/manga/test-manga/chapters"
	);

	assert_eq!(
		Url::manga_covers(TEST_BASE_URL, "test-manga"),
		"https://api.cdnlibs.org/api/manga/test-manga/covers"
	);

	assert_eq!(
		Url::constants(TEST_BASE_URL),
		"https://api.cdnlibs.org/api/constants"
	);

	assert_eq!(
		Url::auth_me(TEST_BASE_URL),
		"https://api.cdnlibs.org/api/auth/me"
	);
}

#[aidoku_test]
fn chapter_pages_url() {
	let url = Url::chapter_pages(TEST_BASE_URL, "test-manga");

	assert_eq!(url, "https://api.cdnlibs.org/api/manga/test-manga/chapter");
}

#[aidoku_test]
fn manga_search_with_params() {
	let url = Url::manga_search_with_params(
		TEST_BASE_URL,
		&[
			("q", "naruto"),
			("bookmarks_exclude[]", "2"),
			("buy", "1"),
			("caution[]", "3"),
			("fields[]", "rate"),
			("fields[]", "rate_avg"),
			("format[]", "6"),
			("genres[]", "41"),
			("licensed", "1"),
			("status[]", "1"),
		],
	);

	assert!(url.starts_with("https://api.cdnlibs.org/api/manga?"));
	assert!(url.contains("q=naruto"));
	assert!(url.contains("buy=1"));
	assert!(url.contains("licensed=1"));
	assert!(url.contains("bookmarks_exclude[]=2"));
}

#[aidoku_test]
fn manga_details_with_fields() {
	let url = Url::manga_details_with_fields(
		TEST_BASE_URL,
		"test-manga",
		&["summary", "tags", "authors", "artists"],
	);

	assert_eq!(
		url,
		"https://api.cdnlibs.org/api/manga/test-manga?fields[]=summary&fields[]=tags&fields[]=authors&fields[]=artists"
	);
}

#[aidoku_test]
fn manga_covers() {
	let url = Url::manga_covers(TEST_BASE_URL, "test-manga");

	assert_eq!(url, "https://api.cdnlibs.org/api/manga/test-manga/covers");
}

#[aidoku_test]
fn chapter_pages_with_params() {
	let url = Url::chapter_pages_with_params(TEST_BASE_URL, "test-manga", Some(123), 1.0, 1.0);

	assert_eq!(
		url,
		"https://api.cdnlibs.org/api/manga/test-manga/chapter?number=1&volume=1&branch_id=123"
	);

	let url_no_branch = Url::chapter_pages_with_params(TEST_BASE_URL, "test-manga", None, 2.5, 2.0);
	assert_eq!(
		url_no_branch,
		"https://api.cdnlibs.org/api/manga/test-manga/chapter?number=2.5&volume=2"
	);
}

#[aidoku_test]
fn constants_with_fields() {
	let url = Url::constants_with_fields(
		TEST_BASE_URL,
		&[
			"genres",
			"tags",
			"scanlateStatus",
			"status",
			"format",
			"ageRestriction",
			"imageServers",
		],
	);

	assert!(url.starts_with("https://api.cdnlibs.org/api/constants?"));
	assert!(url.contains("fields[]=genres"));
	assert!(url.contains("fields[]=tags"));
	assert!(url.contains("fields[]=scanlateStatus"));
	assert!(url.contains("fields[]=status"));
	assert!(url.contains("fields[]=format"));
	assert!(url.contains("fields[]=ageRestriction"));
	assert!(url.contains("fields[]=imageServers"));

	let image_servers_url = Url::constants_with_fields(TEST_BASE_URL, &["imageServers"]);
	assert_eq!(
		image_servers_url,
		"https://api.cdnlibs.org/api/constants?fields[]=imageServers"
	);

	let empty_url = Url::constants_with_fields(TEST_BASE_URL, &[]);
	assert_eq!(empty_url, "https://api.cdnlibs.org/api/constants");
}

#[aidoku_test]
fn trailing_slash_handling() {
	let base_with_slash = "https://api.example.com/";
	let base_without_slash = "https://api.example.com";

	let url1 = Url::manga_search(base_with_slash);
	let url2 = Url::manga_search(base_without_slash);

	assert_eq!(url1, url2);
	assert_eq!(url1, "https://api.example.com/api/manga");
}
