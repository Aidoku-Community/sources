use aidoku::alloc::String;

#[aidoku_test::aidoku_test]
fn listings_always_include_recent_and_conditionally_include_favorites() {
	use crate::listings;
	use aidoku::ListingKind;

	let logged_out = listings(false);
	assert_eq!(logged_out.len(), 1);
	assert_eq!(logged_out[0].id, "recent");
	assert_eq!(logged_out[0].kind, ListingKind::Default);

	let logged_in = listings(true);
	assert_eq!(logged_in.len(), 2);
	assert_eq!(logged_in[0].id, "recent");
	assert_eq!(logged_in[0].kind, ListingKind::Default);
	assert_eq!(logged_in[1].id, "f:fav");
	assert_eq!(logged_in[1].kind, ListingKind::List);
}

#[aidoku_test::aidoku_test]
fn collect_button_page_extracts_nonempty_uuid() {
	use crate::html::CollectButtonPage as _;
	use aidoku::imports::html::Html;

	let document = Html::parse(r#"<button onclick="collect('comic-uuid')"></button>"#).unwrap();
	assert_eq!(document.collect_uuid(), Some(String::from("comic-uuid")));

	let empty = Html::parse(r#"<button onclick="collect('')"></button>"#).unwrap();
	assert_eq!(empty.collect_uuid(), None);

	let missing = Html::parse("<button></button>").unwrap();
	assert_eq!(missing.collect_uuid(), None);
}

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
