use aidoku::{Chapter, Manga, alloc::Vec};

pub fn mentions_mtl(text: &str) -> bool {
	text.split(|ch: char| !ch.is_alphanumeric())
		.any(|word| word.eq_ignore_ascii_case("mtl"))
}

pub fn retain_manga(entries: &mut Vec<Manga>) {
	entries.retain(|manga| !mentions_mtl(&manga.title));
}

pub fn retain_chapters(chapters: &mut Vec<Chapter>) {
	chapters.retain(|chapter| {
		chapter
			.title
			.as_deref()
			.is_none_or(|title| !mentions_mtl(title))
	});
}
