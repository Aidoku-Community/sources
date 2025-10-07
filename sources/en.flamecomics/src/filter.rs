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
	for filter in filters {
		if let FilterValue::Sort { ref ascending, .. } = *filter {
			if *ascending {
				entries.sort_by(|a: &Manga, b: &Manga| b.title.cmp(&a.title));
			} else {
				entries.sort_by(|a: &Manga, b: &Manga| a.title.cmp(&b.title));
			}
		}
	}
	entries
}
