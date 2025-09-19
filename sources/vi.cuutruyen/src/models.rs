// a source made by @c0ntens
use aidoku::{
	alloc::{string::ToString, vec, String, Vec},
	helpers::element::ElementHelpers,
	imports::html::Html,
	prelude::format,
	ContentRating, Manga, MangaStatus, Viewer
};
use serde::Deserialize;

#[derive(Default, Deserialize, Debug, Clone)]
pub struct CuuSearchResponse<T> {
	pub data: T,
	#[serde(rename = "_metadata", default)]
	pub meta: CuuPageCount,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct CuuHome {
	pub spotlight_mangas: Vec<CuuSpotlightManga>,
	pub new_chapter_mangas: Vec<CuuNewestManga>,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct CuuPage {
	pub pages: Vec<CuuPages>,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct CuuPageCount {
	pub total_pages: Option<i32>,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct CuuSpotlightManga {
	pub id: i32,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct CuuManga {
	pub cover_url: Option<String>,
	pub name: String,
	pub id: i32,
}

// idk why some search query got json parsing errors, so i created the same for homepage
#[derive(Default, Deserialize, Debug, Clone)]
pub struct CuuNewestManga {
	pub cover_url: Option<String>,
	pub name: String,
	pub id: i32,
	#[serde(rename = "newest_chapter_created_at")]
	pub created_at: String,
	#[serde(rename = "newest_chapter_id")]
	pub chapter_id: i32,
	#[serde(rename = "newest_chapter_number")]
	pub number: String,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct CuuMangaDetails {
	pub author: CuuNames,
	pub cover_url: Option<String>,
	pub full_description: Option<String>,
	pub is_nsfw: bool,
	pub id: i32,
	pub name: String,
	pub tags: Vec<CuuNames>,
	pub team: CuuNames,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct CuuNames {
	pub name: String,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct CuuChapter {
	pub created_at: String,
	pub id: i32,
	pub number: String,
	pub name: Option<String>,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct CuuPages {
	pub image_url: String,
	pub drm_data: Option<String>,
	pub width: Option<i32>,
	pub height: Option<i32>,
}

impl CuuManga {
	pub fn into_basic_manga(self) -> Manga {
		Manga {
			key: self.id.to_string(),
			title: self.name.to_string(),
			cover: self.cover_url.clone(),
			..Default::default()
		}
	}
}

impl CuuMangaDetails {
	pub fn description(&self) -> Option<String> {
    	if let Some(des) = self.full_description.as_ref() {
        	Html::parse(des).ok().and_then(|doc| {
            	doc.text_with_newlines().map(|text| {
                	if let Some((_, rest)) = text.split_once('\n') {
                    	rest.to_string()
                	} else { text }
            	})
        	})
    	} else {
        	None
    	}
	}

	pub fn authors(&self) -> Option<Vec<String>> {
		Some(vec![self.author.name.to_string()])
	}

	pub fn tags(&self) -> Vec<String> {
		self.tags
			.iter()
			.map(|t| {
				t.name.split_whitespace()
				.map(|word| {
					let mut chars = word.chars();
					match chars.next() {
						None => String::new(),
						Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
					}
				})
				.collect::<Vec<String>>()
				.join(" ")
			})
		.collect()
	}

	pub fn scanlators(&self) -> Option<Vec<String>> {
		Some(vec![self.team.name.to_string()])
	}
}

impl From<CuuMangaDetails> for Manga {
	fn from(val: CuuMangaDetails) -> Self {
		let tags = val.tags();

		let status = if tags.iter().any(|tag| tag == "Đã Hoàn Thành") { MangaStatus::Completed }
		else if tags.iter().any(|tag| tag == "Đang Tiến Hành") { MangaStatus::Ongoing }
		else if tags.iter().any(|tag| tag == "Tạm Ngưng") { MangaStatus::Hiatus }
		else if tags.iter().any(|tag| tag == "Drop") { MangaStatus::Cancelled }
		else { MangaStatus::Unknown };

		let content_rating = if val.is_nsfw || tags.iter().any(|tag| tag == "Khỏa Thân" || tag == "Nsfw" || tag == "Ntr") { ContentRating::NSFW }
		else if tags.iter().any(|tag| tag == "Ecchi") { ContentRating::Suggestive }
		else { ContentRating::Safe };

		let viewer = if tags.iter().any(|tag| tag == "Manhua" || tag == "Manhwa" || tag == "Long Strip" || tag == "Web Comic" || tag == "Webtoon") { Viewer::Webtoon }
		else { Viewer::RightToLeft };

		Manga {
			key: val.id.to_string(),
			title: val.name.to_string(),
			cover: val.cover_url.clone(),
			authors: val.authors(),
			description: val.description(),
			url: Some(format!("https://truycapcuutruyen.pages.dev/mangas/{}", val.id)),
			tags: Some(tags),
			status,
			content_rating,
			viewer,
			..Default::default()
		}
	}
}
