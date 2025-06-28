#![expect(clippy::unwrap_used)]

use super::*;
use aidoku_test::aidoku_test;

#[aidoku_test]
fn manga_31164() {
	let chapters: Vec<Chapter> = serde_json::from_str::<Root>(
		r#"{"list":[{"id":2637675,"isvip":"0","title":"\u7b2c01-02\u8bdd","score":0,"create_time":"2025-06-07 13:03:07"},{"id":2637676,"isvip":"0","title":"\u7b2c03-05\u8bdd","score":0,"create_time":"2025-06-07 13:03:16"},{"id":2637677,"isvip":"0","title":"\u7b2c06\u8bdd","score":0,"create_time":"2025-06-07 13:03:24"},{"id":2637904,"isvip":"0","title":"\u7b2c07\u8bdd","score":0,"create_time":"2025-06-10 14:42:30"},{"id":2637978,"isvip":"0","title":"\u7b2c08\u8bdd","score":0,"create_time":"2025-06-11 14:03:50"},{"id":2638571,"isvip":"0","title":"\u7b2c09\u8bdd","score":0,"create_time":"2025-06-17 14:30:26"},{"id":2638572,"isvip":"0","title":"\u7b2c10\u8bdd","score":0,"create_time":"2025-06-17 14:31:10"},{"id":2638676,"isvip":"0","title":"\u7b2c11\u8bdd","score":0,"create_time":"2025-06-18 13:40:51"},{"id":2639178,"isvip":"0","title":"\u7b2c12\u8bdd","score":0,"create_time":"2025-06-22 12:38:16"}],"history":[]}"#,
	)
	.unwrap()
	.try_into()
	.unwrap();
	assert_eq!(chapters.len(), 9);
	assert_eq!(
		*chapters.first().unwrap(),
		Chapter {
			key: "2639178".into(),
			chapter_number: Some(12.0),
			date_uploaded: Some(1_750_550_400),
			url: Some("https://boylove.cc/home/book/capter/id/2639178".into()),
			..Default::default()
		}
	);
	assert_eq!(
		*chapters.last().unwrap(),
		Chapter {
			key: "2637675".into(),
			title: Some("第01-02话".into()),
			chapter_number: Some(1.0),
			date_uploaded: Some(1_749_254_400),
			url: Some("https://boylove.cc/home/book/capter/id/2637675".into()),
			..Default::default()
		}
	);
}
