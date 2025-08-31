// use crate::helper;
// use crate::model::SortOptions;
use aidoku::{
	alloc::{ string::ToString, String, Vec},
	helpers::uri::QueryParameters,
	prelude::*,
	FilterValue,
};

use crate::model::SortOptions;
/* FilterValues
 * Add all filters to corresponding value(ex: Sort: sort(option), order(ascending))

 * Text: 
 * Sort: (Order By: Rating Score, Most Follows, Most Reviews, Most Comments, Most Chapters, New Chapters, Recently Created, Name A-Z, )
 *  - By Views(60 min, 6 hrs, 12 hrs, 24 hrs, 7 days, 30 days, 90 days)
 * Select: (Original Work Status, MPark Upload Status, Number of Chapters))
 * Check: (Translated Language)
 * MultiSelect: (genres, Original Work Language)
 * Range:
 

 36 manga/page
 * Note: genres => include, exclude
*/
pub fn get_filters(query: Option<String>, filters: Vec<FilterValue>) -> String {
	let mut qs = QueryParameters::new();

	if query.is_some() {
		qs.push("word", query.as_deref()); 
	}
	for filter in filters {
		match filter {
			FilterValue::MultiSelect {
				ref id,
				ref included,
				ref excluded,
			} => {
				if id == "genre" {
					let mut joined: String = "".to_string();
					if !included.is_empty() {
						joined = included.join(",");
					}	
					if !excluded.is_empty() { 
						joined.push_str("|");
						joined = joined + &excluded.join(",");
					}
					if !included.is_empty() || !excluded.is_empty() {
						qs.push("genres", Some(&joined));
					}
				}
				else{
					qs.push("orig", Some(&included.join(",")));
				}
			}
			FilterValue::Select{
				id, value
			} => {
				if id == "original_work_status"{
					qs.push("status", Some(&value));
				}
				if id == "mpark_upload_status" {
					qs.push("upload", Some(&value));
				}
				if id == "chapters"{
					qs.push("chapters", Some(&value));
				}
			}
			FilterValue::Sort {
				index, ..
			} => {
				let option: &str = SortOptions::from(index).into();
				qs.push("sortby", Some(option));
			}
			_ => {}
		}
	}

	format!("{qs}")
}
