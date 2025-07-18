#![expect(clippy::unwrap_used)]

use super::*;
use aidoku_test::aidoku_test;

#[aidoku_test]
fn filters_default() {
	assert_eq!(
		Url::from_query_or_filters(None, 1, &[])
			.unwrap()
			.to_string(),
		"https://www.2025copy.com/comics?ordering=-datetime_updated&offset=0&limit=50"
	);
}

#[aidoku_test]
fn filters_romance_manga_ongoing_popularity_ascending_2() {
	assert_eq!(
		Url::from_query_or_filters(
			None,
			2,
			&[
				FilterValue::Select {
					id: "地區".into(),
					value: "0".into()
				},
				FilterValue::Select {
					id: "狀態".into(),
					value: "0".into()
				},
				FilterValue::Sort {
					id: "排序".into(),
					index: 1,
					ascending: true
				},
				FilterValue::Select {
					id: "題材".into(),
					value: "aiqing".into()
				}
			]
		)
		.unwrap()
		.to_string(),
		"https://www.2025copy.com/comics?theme=aiqing&status=0&region=0&ordering=popular&offset=50&limit=50"
	);
}

// #[aidoku_test]
// fn filters_author() {
// 	let url = Url::from_query_or_filters(
// 		None,
// 		1,
// 		&[FilterValue::Text {
// 			id: "author".into(),
// 			value: "アシダカヲズ".into(),
// 		}],
// 	)
// 	.unwrap();
// 	assert_eq!(
// 		url.to_string(),
// 		"https://www.2025copy.com/api/kb/web/searchbh/comics?offset=0&platform=2&limit=12&q=%E3%82%A2%E3%82%B7%E3%83%80%E3%82%AB%E3%83%B2%E3%82%BA&q_type=author"
// 	);
// 	assert!(
// 		url.request()
// 			.unwrap()
// 			.string()
// 			.unwrap()
// 			.starts_with(r#"{"code":200"#)
// 	);
// }

// #[aidoku_test]
// fn query_red_1() {
// 	let url = Url::from_query_or_filters(Some("紅"), 1, &[]).unwrap();
// 	assert_eq!(
// 		url.to_string(),
// 		"https://www.2025copy.com/api/kb/web/searchbh/comics?offset=0&platform=2&limit=12&q=%E7%B4%85&q_type="
// 	);
// 	assert!(
// 		url.request()
// 			.unwrap()
// 			.string()
// 			.unwrap()
// 			.starts_with(r#"{"code":200"#)
// 	);
// }

// #[aidoku_test]
// fn query_blue_2() {
// 	let url = Url::from_query_or_filters(Some("藍"), 2, &[]).unwrap();
// 	assert_eq!(
// 		url.to_string(),
// 		"https://www.2025copy.com/api/kb/web/searchbh/comics?offset=12&platform=2&limit=12&q=%E8%97%8D&q_type="
// 	);
// 	assert!(
// 		url.request()
// 			.unwrap()
// 			.string()
// 			.unwrap()
// 			.starts_with(r#"{"code":200"#)
// 	);
// }

#[aidoku_test]
fn manga() {
	assert_eq!(
		Url::manga("heishoudangbaomu").to_string(),
		"https://www.2025copy.com/comic/heishoudangbaomu"
	);
}

#[aidoku_test]
fn chapter_list() {
	let url = Url::chapter_list("wodelinjuzhongshidingzhewo");
	assert_eq!(
		url.to_string(),
		"https://www.2025copy.com/comicdetail/wodelinjuzhongshidingzhewo/chapters"
	);
	assert!(
		url.request()
			.unwrap()
			.string()
			.unwrap()
			.starts_with(r#"{"code":200"#)
	);
}

#[aidoku_test]
fn chapter() {
	assert_eq!(
		Url::chapter("jiandieguojiajia", "555d889e-98ab-11ea-ad89-00163e0ca5bd").to_string(),
		"https://www.2025copy.com/comic/jiandieguojiajia/chapter/555d889e-98ab-11ea-ad89-00163e0ca5bd"
	);
}
