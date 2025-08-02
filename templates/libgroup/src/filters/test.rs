use aidoku::alloc::vec;
use aidoku_test::aidoku_test;

use super::*;

fn create_processor() -> FilterProcessor {
	FilterProcessor::new()
}

#[aidoku_test]
fn test_empty_filters() {
	let processor = create_processor();
	let result = processor.process_filters(vec![]);
	assert!(result.is_empty());
}

#[aidoku_test]
fn test_text_filter_validation() {
	let processor = create_processor();

	let filters = vec![
		FilterValue::Text {
			id: "chap_count_min".to_string(),
			value: "10".to_string(),
		},
		FilterValue::Text {
			id: "year_max".to_string(),
			value: "invalid".to_string(), // Should be filtered out
		},
		FilterValue::Text {
			id: "rating_min".to_string(),
			value: "5".to_string(),
		},
		FilterValue::Text {
			id: "rate_max".to_string(),
			value: "8".to_string(),
		},
	];

	let result = processor.process_filters(filters);

	assert_eq!(result.len(), 3);
	assert!(result.contains(&("chap_count_min", "10".to_string())));
	assert!(result.contains(&("rating_min", "5".to_string())));
	assert!(result.contains(&("rate_max", "8".to_string())));
}

#[aidoku_test]
fn test_sort_filter_default_behavior() {
	let processor = create_processor();

	// Default sort (popularity, descending) should be skipped
	let default_sort = vec![FilterValue::Sort {
		id: "sort".to_string(),
		index: 0,
		ascending: false,
	}];

	let result = processor.process_filters(default_sort);
	assert!(result.is_empty());

	// Non-default sort should be included
	let custom_sort = vec![FilterValue::Sort {
		id: "sort".to_string(),
		index: 1, // Rating
		ascending: true,
	}];

	let result = processor.process_filters(custom_sort);
	assert_eq!(result.len(), 2);
	assert!(result.contains(&("sort_by", "rate_avg".to_string())));
	assert!(result.contains(&("sort_type", "asc".to_string())));
}

#[aidoku_test]
fn test_check_filter_inversion() {
	let processor = create_processor();

	let filters = vec![
		FilterValue::Check {
			id: "genres_soft_search".to_string(),
			value: 0, // Unchecked should enable soft search
		},
		FilterValue::Check {
			id: "tags_soft_search".to_string(),
			value: 1, // Checked should be ignored
		},
	];

	let result = processor.process_filters(filters);

	assert_eq!(result.len(), 1);
	assert!(result.contains(&("genres_soft_search", "1".to_string())));
}

#[aidoku_test]
fn test_multiselect_with_exclusions() {
	let processor = create_processor();

	let filters = vec![FilterValue::MultiSelect {
		id: "genres".to_string(),
		included: vec!["Action".to_string(), "Comedy".to_string()],
		excluded: vec!["Horror".to_string()],
	}];

	let result = processor.process_filters(filters);

	assert_eq!(result.len(), 3);
	assert!(result.contains(&("genres[]", "Action".to_string())));
	assert!(result.contains(&("genres[]", "Comedy".to_string())));
	assert!(result.contains(&("genres_exclude[]", "Horror".to_string())));
}

#[aidoku_test]
fn test_multiselect_without_exclusions() {
	let processor = create_processor();

	let filters = vec![FilterValue::MultiSelect {
		id: "age_rating".to_string(),
		included: vec!["1".to_string(), "3".to_string()],
		excluded: vec!["2".to_string()], // Should be ignored for age_rating
	}];

	let result = processor.process_filters(filters);

	assert_eq!(result.len(), 2);
	assert!(result.contains(&("caution[]", "1".to_string())));
	assert!(result.contains(&("caution[]", "3".to_string())));
	// Excluded values should not appear for age_rating
	assert!(!result.iter().any(|(key, _)| key.contains("exclude")));
}
