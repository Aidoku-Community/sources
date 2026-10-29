use crate::html::parse_chapter_number;

#[aidoku_test::aidoku_test]
fn parse_chapter_number_handles_arabic_numerals() {
	assert_eq!(parse_chapter_number("第12话 标题"), Some(12.0));
	assert_eq!(parse_chapter_number("第12.5话"), Some(12.5));
}

#[aidoku_test::aidoku_test]
fn parse_chapter_number_handles_chinese_numerals() {
	assert_eq!(parse_chapter_number("第一章"), Some(1.0));
	assert_eq!(parse_chapter_number("第十二章"), Some(12.0));
	assert_eq!(parse_chapter_number("第一百二十三章"), Some(123.0));
	// Regression: the section part was multiplied together with `百`/`千`,
	// so 一千二百 used to come out as 100200.
	assert_eq!(parse_chapter_number("第一千二百章"), Some(1200.0));
	assert_eq!(parse_chapter_number("第两千零五章"), Some(2005.0));
}

#[aidoku_test::aidoku_test]
fn parse_chapter_number_ignores_titles_without_a_number() {
	assert_eq!(parse_chapter_number("序章"), None);
	assert_eq!(parse_chapter_number("终章"), None);
	assert_eq!(parse_chapter_number("番外"), None);
}

#[aidoku_test::aidoku_test]
fn deep_link_recognises_articleinfo_urls() {
	use crate::Wenku8;
	use aidoku::{DeepLinkHandler as _, DeepLinkResult, alloc::String};

	let source = Wenku8;
	let detail = |url: &str| source.handle_deep_link(String::from(url)).unwrap();

	assert!(matches!(
		detail("https://www.wenku8.net/modules/article/articleinfo.php?id=2070&charset=gbk"),
		Some(DeepLinkResult::Manga { key }) if key == "2070"
	));
	assert!(detail("https://www.wenku8.net/modules/article/articleinfo.php").is_none());
	assert!(detail("https://www.wenku8.net/modules/article/articleinfo.php?id=abc").is_none());
	assert!(matches!(
		detail("https://www.wenku8.net/book/2070.htm#comments"),
		Some(DeepLinkResult::Manga { key }) if key == "2070"
	));
}
