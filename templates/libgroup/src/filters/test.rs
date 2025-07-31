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
fn test_select_filter_inversion() {
	let processor = create_processor();

	let filters = vec![
		FilterValue::Select {
			id: "genres_match_mode".to_string(),
			value: "any".to_string(), // Unchecked should enable soft search
		},
		FilterValue::Select {
			id: "tags_match_mode".to_string(),
			value: "all".to_string(), // Checked should be ignored
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

#[aidoku_test]
fn test_range_filter_basic() {
	let processor = create_processor();
	let filters = vec![
		FilterValue::Range {
			id: "chap_count".to_string(),
			from: Some(10.0),
			to: Some(50.0),
		},
		FilterValue::Range {
			id: "year".to_string(),
			from: Some(2025.0),
			to: None,
		},
	];

	let result = processor.process_filters(filters);
	assert_eq!(result.len(), 3);
	assert!(result.contains(&("chap_count_min", "10".to_string())));
	assert!(result.contains(&("chap_count_max", "50".to_string())));
	assert!(result.contains(&("year_min", "2025".to_string())));
}
