#![expect(clippy::unwrap_used)]

use super::*;
use aidoku_test::aidoku_test;

#[aidoku_test]
fn filters_default() {
	assert_eq!(
		Url::from_filters(1, &[]).unwrap().to_string(),
		"https://www.2025copy.com/comics?ordering=-datetime_updated&offset=0&limit=50"
	);
}

#[aidoku_test]
fn filters_romance_manga_ongoing_popularity_ascending_2() {
	assert_eq!(
		Url::from_filters(
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
