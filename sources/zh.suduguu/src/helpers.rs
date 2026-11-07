use crate::html;
use crate::net::Url;
use aidoku::{
	alloc::String,
	imports::{html::Document, std::sleep},
};

/// Number of request attempts per paginated chapter page (initial + retries).
const MAX_ATTEMPTS: u32 = 3;

/// Fetch one paginated chapter page, retrying with increasing backoff.
pub fn fetch_chapter_page(book: &str, chapter: &str, page: i32) -> Option<Document> {
	let mut attempt: u32 = 0;
	loop {
		let result = Url::chapter_page(book, chapter, page)
			.request()
			.and_then(|r| Ok(r.html()?));
		match result {
			Ok(document) => return Some(document),
			Err(_) => {
				attempt += 1;
				if attempt >= MAX_ATTEMPTS {
					return None;
				}
				sleep(2 * attempt as i32);
			}
		}
	}
}

/// Extract the body text of one chapter page. `None` when the `.con`
/// container is missing; an empty string means the body is empty (a common
/// rate-limit symptom, retried by the caller).
pub fn extract_page_text(document: &Document, page_no: i32) -> Option<String> {
	let content = document.select_first(".con")?;
	let raw = content.html().unwrap_or_default();
	let text = html::chapter_text(&raw);
	// Only the first page repeats the chapter title as the leading paragraph.
	let text = if page_no == 1 {
		match html::chapter_title(document) {
			Some(title) => html::strip_leading_title(&text, &title),
			None => text,
		}
	} else {
		text
	};
	Some(text)
}

pub fn is_numeric(s: &str) -> bool {
	!s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}
