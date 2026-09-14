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
fn detail_actions_keep_favorite_and_comment_section_on_one_line() {
	let actions = crate::favorites::detail_action_line(
		Some("[➕ 收藏漫画](https://example.com/add)"),
		Some("comic-uuid"),
	)
	.expect("actions should be present");
	assert_eq!(
		actions,
		"[💬 评论区](https://www.copy5000.com/h5/commentList?comicId=comic-uuid) · [➕ 收藏漫画](https://example.com/add)"
	);
}

#[aidoku_test::aidoku_test]
fn favorite_buttons_use_the_normal_copy5000_domain() {
	assert_eq!(crate::favorites::button_host(), "https://www.copy5000.com");
}

#[aidoku_test::aidoku_test]
fn newest_page_reads_the_discovery_page_cards() {
	use crate::html::NewestPage as _;
	use aidoku::imports::html::Html;

	let document = Html::parse(
		r#"<div class="exemptComic_Item">
			<a href="/comic/new-comic"><img data-src="https://img.example/cover.jpg.328x422.jpg"></a>
			<div class="exemptComicItem-txt"><a href="/comic/new-comic"><p>全新作品</p></a><span><a>作者甲</a></span></div>
		</div><li class="page-all-item active"><a>1</a></li><li class="page-all-item"><a>2</a></li>"#,
	)
	.unwrap();
	let page = document.newest_manga_page_result().unwrap();
	assert_eq!(page.entries.len(), 1);
	assert_eq!(page.entries[0].key, "new-comic");
	assert_eq!(page.entries[0].title, "全新作品");
	assert_eq!(
		page.entries[0].cover.as_deref(),
		Some("https://img.example/cover.jpg")
	);
	assert!(page.has_next_page);
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
