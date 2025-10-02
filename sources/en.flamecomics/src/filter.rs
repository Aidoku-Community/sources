use aidoku::{
	FilterValue, Manga,
	alloc::{String, Vec, string::ToString},
};

pub fn get_genre_filter(filters: &Vec<FilterValue>) -> String {
	let mut filter_str: String = "".to_string();
	for filter in filters {
		if let FilterValue::Select { ref id, ref value } = *filter
			&& id == "genre"
		{
			filter_str.push_str("genre/");
			filter_str.push_str(value);
		}
	}
	filter_str.to_string()
}

pub fn sort(filters: &Vec<FilterValue>, mut entries: Vec<Manga>) -> Vec<Manga> {
	let mut sort_by_ascending: bool = true;
	for filter in filters {
		if let FilterValue::Sort { ref ascending, .. } = *filter {
			sort_by_ascending = *ascending;
		}
	}
	if sort_by_ascending {
		entries.sort_by(|a, b| a.title.cmp(&b.title));
	} else {
		entries.sort_by(|a, b| b.title.cmp(&a.title));
	}
	entries
}
