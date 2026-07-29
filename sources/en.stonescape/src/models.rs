use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, MangaWithChapter, Viewer,
	alloc::{String, Vec, format},
};
use serde::Deserialize;

use crate::BASE_URL;

fn format_genre(genre: &str) -> String {
	match genre {
		"action" => "Action".into(),
		"adaptation" => "Adaptation".into(),
		"adult" => "Adult".into(),
		"adventure" => "Adventure".into(),
		"comedy" => "Comedy".into(),
		"demons" => "Demons".into(),
		"drama" => "Drama".into(),
		"ecchi" => "Ecchi".into(),
		"fantasy" => "Fantasy".into(),
		"genderbender" | "gender-bender" => "Gender Bender".into(),
		"gore" => "Gore".into(),
		"harem" => "Harem".into(),
		"historical" => "Historical".into(),
		"horror" => "Horror".into(),
		"isekai" => "Isekai".into(),
		"josei" => "Josei".into(),
		"magic" => "Magic".into(),
		"martialarts" | "martial-arts" => "Martial Arts".into(),
		"mature" => "Mature".into(),
		"mecha" => "Mecha".into(),
		"military" => "Military".into(),
		"monsters" => "Monsters".into(),
		"mystery" => "Mystery".into(),
		"post-apocalyptic" => "Post-Apocalyptic".into(),
		"psychological" => "Psychological".into(),
		"romance" => "Romance".into(),
		"schoollife" | "school-life" => "School Life".into(),
		"sci-fi" | "scifi" => "Sci-Fi".into(),
		"seinen" => "Seinen".into(),
		"shoujo" => "Shoujo".into(),
		"shoujoai" | "shoujo-ai" => "Shoujo Ai".into(),
		"shounen" => "Shounen".into(),
		"shounenai" | "shounen-ai" => "Shounen Ai".into(),
		"sliceoflife" | "slice-of-life" => "Slice of Life".into(),
		"smut" => "Smut".into(),
		"sports" => "Sports".into(),
		"supernatural" => "Supernatural".into(),
		"thriller" => "Thriller".into(),
		"tragedy" => "Tragedy".into(),
		"video-games" | "videogames" => "Video Games".into(),
		"webtoons" | "webtoon" => "Webtoons".into(),
		"wuxia" => "Wuxia".into(),
		"yaoi" => "Yaoi".into(),
		"yuri" => "Yuri".into(),
		_ => {
			let mut s = String::from(genre);
			if let Some(r) = s.get_mut(0..1) {
				r.make_ascii_uppercase();
			}
			s
		}
	}
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BannerResponse {
	pub featured_series: Vec<Series>,
}

#[derive(Deserialize)]
pub struct SeriesResponse {
	pub data: Vec<Series>,
	pub pagination: Option<Pagination>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
	pub page: Option<i32>,
	pub total_pages: Option<i32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Series {
	pub title: String,
	pub slug: String,
	pub cover_url: Option<String>,
	pub banner_url: Option<String>,
	pub description: Option<String>,
	pub publication_status: Option<String>,
	pub author: Option<String>,
	pub artist: Option<String>,
	pub genres: Option<Vec<String>>,
	pub latest_chapter: Option<ChapterData>,
}

impl Series {
	fn apply_genres(&self, manga: &mut Manga) {
		if let Some(genres) = &self.genres {
			let is_nsfw = genres.iter().any(|g| {
				g.eq_ignore_ascii_case("adult")
					|| g.eq_ignore_ascii_case("gore")
					|| g.eq_ignore_ascii_case("mature")
					|| g.eq_ignore_ascii_case("smut")
			});
			let is_suggestive = genres.iter().any(|g| g.eq_ignore_ascii_case("ecchi"));

			manga.content_rating = if is_nsfw {
				ContentRating::NSFW
			} else if is_suggestive {
				ContentRating::Suggestive
			} else {
				ContentRating::Safe
			};

			manga.tags = Some(genres.iter().map(|g| format_genre(g)).collect());
		}
	}

	pub fn into_manga(self) -> Manga {
		let cover = self.cover_url.map(|path| format!("{BASE_URL}{path}"));

		Manga {
			key: self.slug,
			title: self.title,
			cover,
			..Default::default()
		}
	}

	pub fn into_banner_manga(self) -> Manga {
		let cover = self
			.banner_url
			.or(self.cover_url)
			.map(|path| format!("{BASE_URL}{path}"));

		Manga {
			key: self.slug,
			title: self.title,
			cover,
			description: self.description,
			..Default::default()
		}
	}

	pub fn into_manga_with_chapter(mut self) -> Option<MangaWithChapter> {
		let slug = String::from(self.slug.as_str());
		let chapter = self.latest_chapter.take()?.into_chapter(&slug);
		let manga = self.into_manga();
		Some(MangaWithChapter { manga, chapter })
	}

	pub fn apply_details(self, manga: &mut Manga) {
		self.apply_genres(manga);
		manga.title = self.title;
		manga.description = self.description;
		manga.cover = self.cover_url.map(|path| format!("{BASE_URL}{path}"));
		manga.url = Some(format!("{BASE_URL}/series/{}", self.slug));
		manga.authors = self
			.author
			.filter(|a| !a.is_empty())
			.map(|a| aidoku::alloc::vec![a]);
		manga.artists = self
			.artist
			.filter(|a| !a.is_empty())
			.map(|a| aidoku::alloc::vec![a]);

		manga.status = match self.publication_status.as_deref() {
			Some("ongoing") => MangaStatus::Ongoing,
			Some("completed") => MangaStatus::Completed,
			Some("hiatus") => MangaStatus::Hiatus,
			Some("dropped") | Some("cancelled") => MangaStatus::Cancelled,
			_ => MangaStatus::Unknown,
		};

		manga.viewer = Viewer::Webtoon;
	}
}

#[derive(Deserialize)]
pub struct ChapterListResponse {
	pub chapters: Vec<ChapterData>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterData {
	pub chapter_id: String,
	pub chapter_number: String,
	pub title: Option<String>,
	pub created_at: Option<String>,
}

impl ChapterData {
	pub fn into_chapter(self, slug: &str) -> Chapter {
		let chapter_number = self.chapter_number.parse::<f32>().ok();
		let url = format!(
			"{BASE_URL}/series/{slug}/ch-{}#{}",
			self.chapter_number, self.chapter_id
		);

		let title = self.title.filter(|t| !t.is_empty());

		let date_uploaded = self
			.created_at
			.and_then(|dt| chrono::DateTime::parse_from_rfc3339(&dt).ok())
			.map(|d| d.timestamp());

		Chapter {
			key: self.chapter_id,
			title,
			chapter_number,
			date_uploaded,
			url: Some(url),
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct ChapterDetails {
	pub pages: Option<Vec<PageData>>,
	pub images: Option<Vec<PageData>>,
}

#[derive(Deserialize)]
pub struct PageData {
	pub url: String,
}
