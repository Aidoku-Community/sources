use crate::BASE_URL;
use crate::models::ListComicResponse;
use aidoku::{
	Manga, Result,
	alloc::{Vec, format, vec},
	imports::net::Request,
};

pub fn to_manga_status(status_str: &str) -> aidoku::MangaStatus {
	match status_str {
		"En cours" => aidoku::MangaStatus::Ongoing,
		"Terminé" => aidoku::MangaStatus::Completed,
		_ => aidoku::MangaStatus::Unknown,
	}
}

pub fn to_manga_content_rating(adult: i32) -> aidoku::ContentRating {
	match adult {
		0 => aidoku::ContentRating::Safe,
		1 => aidoku::ContentRating::NSFW,
		_ => aidoku::ContentRating::Unknown,
	}
}

pub fn get_all_mangas(sorted_by_updated: bool) -> Result<Vec<Manga>> {
	let response: ListComicResponse =
		Request::get(format!("{BASE_URL}/api/comics"))?.json_owned()?;

	let mut comics = response.comics;
	if sorted_by_updated {
		comics.sort_by(|a, b| {
			let a_date = a
				.last_chapter
				.as_ref()
				.and_then(|c| c.published_on.as_deref())
				.or(Some(a.updated_at.as_str()))
				.unwrap_or("");

			let b_date = b
				.last_chapter
				.as_ref()
				.and_then(|c| c.published_on.as_deref())
				.or(Some(b.updated_at.as_str()))
				.unwrap_or("");

			b_date.cmp(a_date)
		});
	}

	let mangas = comics
		.into_iter()
		.map(|comic| Manga {
			key: comic.slug,
			title: comic.title,
			description: comic.description,
			url: Some(format!("{BASE_URL}{}", comic.url)),
			cover: Some(comic.thumbnail),
			authors: comic.author.filter(|a| !a.is_empty()).map(|a| vec![a]),
			artists: comic.artist.filter(|a| !a.is_empty()).map(|a| vec![a]),
			viewer: aidoku::Viewer::RightToLeft,
			content_rating: to_manga_content_rating(comic.adult),
			status: to_manga_status(&comic.status),
			..Default::default()
		})
		.collect::<Vec<_>>();

	Ok(mangas)
}
