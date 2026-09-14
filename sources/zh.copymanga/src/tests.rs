use aidoku::alloc::String;

#[aidoku_test::aidoku_test]
fn favorite_deep_links_require_a_known_operation_and_comic_id() {
	use crate::parse_fav_deep_link;

	assert_eq!(
		parse_fav_deep_link("https://www.copy5000.com/__fav/add/abc123"),
		Some((String::from("abc123"), true))
	);
	assert_eq!(
		parse_fav_deep_link("https://www.copy5000.com/__fav/remove/abc123"),
		Some((String::from("abc123"), false))
	);
	assert_eq!(
		parse_fav_deep_link("https://www.copy5000.com/__fav/add/"),
		None
	);
	assert_eq!(
		parse_fav_deep_link("https://www.copy5000.com/__fav/toggle/abc123"),
		None
	);
}
