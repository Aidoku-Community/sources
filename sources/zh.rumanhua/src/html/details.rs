use crate::net::{extract_chapter_key, extract_chapter_number, get_absolute_url};
use aidoku::{
	Chapter, Manga, MangaStatus, Result,
	alloc::{String, Vec, format},
	imports::html::Document,
};

pub trait RumanhuaDetailsHtml {
	fn update_details(&self, manga: &mut Manga) -> Result<()>;
	fn get_chapters(&self) -> Result<Vec<Chapter>>;
}

impl RumanhuaDetailsHtml for Document {
	fn update_details(&self, manga: &mut Manga) -> Result<()> {
		if let Some(img) = self.select_first("div.detailTop div.content img.cover") {
			let cover = img.attr("src").unwrap_or_default();
			let title = img.attr("alt").unwrap_or_default();
			if !cover.is_empty() {
				manga.cover = Some(get_absolute_url(&cover));
			}
			if !title.is_empty() {
				manga.title = title;
			}
		}

		if let Some(p) = self.select_first("div.detailContent p") {
			let desc = p.text().unwrap_or_default();
			if !desc.is_empty() {
				manga.description = Some(desc);
			}
		}

		let mut authors = Vec::new();
		let mut status = MangaStatus::Unknown;

		if let Some(ps) = self.select("div.detailTop div.info p.subtitle") {
			for p in ps {
				let text = p.text().unwrap_or_default();
				let clean = text.trim().replace(['\u{00a0}', '\u{3000}'], "");
				if let Some(author) = clean.strip_prefix("作者：") {
					let author = author.trim();
					if !author.is_empty() {
						authors.push(String::from(author));
					}
				} else if clean.starts_with("状态：") || clean.starts_with("更新至：") {
					status = if clean.contains("完结") {
						MangaStatus::Completed
					} else {
						MangaStatus::Ongoing
					};
				}
			}
		}

		if !authors.is_empty() {
			manga.authors = Some(authors);
		}
		manga.status = status;
		manga.url = Some(get_absolute_url(&format!("/news/{}", manga.key)));
		Ok(())
	}

	fn get_chapters(&self) -> Result<Vec<Chapter>> {
		let mut chapters = Vec::new();

		if let Some(elements) = self.select("ul.chapterList li a") {
			for a in elements {
				let url = a.attr("href").unwrap_or_default();
				let title = a.text().unwrap_or_default();
				let Some(key) = extract_chapter_key(&url) else {
					continue;
				};
				let chapter_num = extract_chapter_number(&title).unwrap_or(0.0);

				chapters.push(Chapter {
					key,
					title: Some(title),
					chapter_number: Some(chapter_num),
					url: Some(get_absolute_url(&url)),
					..Default::default()
				});
			}
		}

		// Mobile site lists oldest→newest; Aidoku expects newest→oldest
		chapters.reverse();

		Ok(chapters)
	}
}
