use crate::{helper::ElementImageAttr, Params};
use aidoku::{
	alloc::{borrow::ToOwned, String, Vec},
	imports::html::Document,
	prelude::*,
	AidokuError, Chapter, ContentRating, Manga, MangaStatus, Result, Viewer,
};

pub fn parse_response<T: AsRef<str>>(
	html: &Document,
	base_url: &str,
	item_selector: T,
) -> Vec<Manga> {
	html.select(&item_selector)
		.map(|x| {
			x.filter_map(|element| {
				let href = element.attr("href")?;
				let key = href
					.strip_prefix(base_url)
					.map(String::from)
					.unwrap_or(href);
				let img = element.select_first("img")?;
				let title = img.attr("alt")?;
				let cover = img.attr("abs:src");

				Some(Manga {
					key,
					title,
					cover,
					..Default::default()
				})
			})
			.collect::<Vec<Manga>>()
		})
		.unwrap_or_default()
}

pub fn parse_manga_details(manga: &mut Manga, html: &Document) -> Result<()> {
	let element = html
		.select_first("#ani_detail")
		.ok_or(AidokuError::message("Unable to find manga details"))?;

	manga.title = element
		.select_first(".manga_name")
		.and_then(|e| e.own_text())
		.unwrap_or(manga.title.clone());
	manga.cover = element.select_first("img").and_then(|img| img.img_attr());

	let (authors, artists) = element
		.select(".anisc-info > .item:contains(Author), .anisc-info > .item:contains(著者)")
		.and_then(|authors_element| {
			let text = authors_element.text()?;
			let author_names = authors_element.select("a")?.filter_map(|el| el.own_text());

			let mut authors = Vec::new();
			let mut artists = Vec::new();
			for author in author_names {
				let is_artist = text.contains(&format!("{author} (Art)"));
				let name = author.replace(",", "");
				if is_artist {
					artists.push(name);
				} else {
					authors.push(name);
				}
			}
			Some((Some(authors), Some(artists)))
		})
		.unwrap_or((None, None));
	manga.authors = authors;
	manga.artists = artists;

	manga.description = element
		.select_first(".description")
		.and_then(|e| e.own_text());
	manga.tags = element
		.select(".genres > a")
		.map(|els| els.filter_map(|el| el.own_text()).collect());
	manga.status = element
		.select_first(
			".anisc-info > .item:contains(Status) .name, .anisc-info > .item:contains(地位) .name",
		)
		.and_then(|el| el.text())
		.map(|status| match status.to_lowercase().as_str() {
			"ongoing" | "publishing" | "releasing" => MangaStatus::Ongoing,
			"completed" | "finished" => MangaStatus::Completed,
			"on-hiatus" | "on hiatus" => MangaStatus::Hiatus,
			"canceled" | "discontinued" => MangaStatus::Cancelled,
			_ => MangaStatus::Unknown,
		})
		.unwrap_or_default();

	let tags = manga.tags.as_deref().unwrap_or(&[]);
	manga.content_rating = if tags.iter().any(|e| e == "Hentai" || e == "エロい") {
		ContentRating::NSFW
	} else if tags.iter().any(|e| e == "Ecchi") {
		ContentRating::Suggestive
	} else if element
		.select_first(".anisc-info > .item:contains(タイプ) .name")
		.and_then(|el| el.text())
		.is_some_and(|t| t == "オトナコミック")
	{
		ContentRating::NSFW
	} else {
		ContentRating::Safe
	};

	manga.viewer = element
		.select_first(".anisc-info > .item:contains(Type) .name")
		.and_then(|el| el.text())
		.map(|status| match status.to_lowercase().as_str() {
			"manhwa" | "manhua" => Viewer::Webtoon,
			"comic" => Viewer::LeftToRight,
			_ => Viewer::RightToLeft,
		})
		.unwrap_or(Viewer::RightToLeft);

	Ok(())
}

pub fn parse_manga_chapters(html: &Document, params: &Params) -> Option<Vec<Chapter>> {
	html.select((params.get_chapter_selector)()).map(|els| {
		let mut c = els
			.filter_map(|el| {
				let link = el.select_first("a")?;
				let url = link.attr("abs:href")?;
				let mut key: String = url.strip_prefix(params.base_url.as_ref())?.into();
				if let Some(id) = el.attr("data-id") {
					key.push_str(&format!("#{id}"));
				}
				let mut title = link.select_first(".name").and_then(|el| el.text());
				let chapter_number =
					title
						.as_ref()
						.and_then(|title| title.find(':'))
						.and_then(|colon| {
							let chapter_num_text = &title.as_ref().unwrap()[..colon].to_owned();
							title = Some(title.as_ref().unwrap()[colon + 1..].trim().into());
							chapter_num_text
								.chars()
								.filter(|c| c.is_ascii_digit() || *c == '.')
								.collect::<String>()
								.parse::<f32>()
								.ok()
						});
				if title.as_ref().is_some_and(|t| {
					*t == format!("Chapter {}", chapter_number.unwrap_or_default())
						|| *t == format!("第{}話", chapter_number.unwrap_or_default())
						|| *t == format!("第 {} 話", chapter_number.unwrap_or_default())
						|| *t == format!("【第 {} 話】", chapter_number.unwrap_or_default())
				}) {
					title = None;
				}
				let language = (params.get_chapter_language)(&el);
				Some(Chapter {
					key,
					title,
					chapter_number,
					url: Some(url),
					language: language.into(),
					..Default::default()
				})
			})
			.collect::<Vec<_>>();
		// sort combined chapters by chapter number
		// since separate languages are grouped together by default
		c.sort_by(|a, b| {
			let a_num = a.chapter_number.unwrap_or(-1.0);
			let b_num = b.chapter_number.unwrap_or(-1.0);
			b_num
				.partial_cmp(&a_num)
				.unwrap_or(core::cmp::Ordering::Equal)
		});
		c
	})
}

pub fn parse_manga_list(html: &Document, base_url: &str) -> Vec<Manga> {
	html.select(".item")
		.map(|els| {
			els.filter_map(|e| {
				let link_href = e.select_first("a.manga-poster")?.attr("href")?;
				Some(Manga {
					key: link_href
						.strip_prefix(base_url)
						.map(|s| s.into())
						.unwrap_or(link_href),
					title: e.select_first(".manga-name")?.text()?,
					cover: e.select_first(".manga-poster img")?.attr("src"),
					..Default::default()
				})
			})
			.collect()
		})
		.unwrap_or_default()
}
