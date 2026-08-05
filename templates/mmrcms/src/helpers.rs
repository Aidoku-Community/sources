use aidoku::{
	Manga, MangaStatus,
	alloc::{String, format},
	imports::html::Element,
};

pub trait ElementImageAttr {
	fn img_attr(&self) -> Option<String>;
}

impl ElementImageAttr for Element {
	fn img_attr(&self) -> Option<String> {
		self.attr("abs:data-background-image")
			.or_else(|| self.attr("abs:data-cfsrc"))
			.or_else(|| self.attr("abs:data-lazy-src"))
			.or_else(|| self.attr("abs:data-src"))
			.or_else(|| self.attr("abs:src"))
	}
}

pub fn manga_from_element(element: &Element, base_url: &str, item_path: &str) -> Option<Manga> {
	let anchor = element.select_first(".media-heading a, .manga-heading a")?;
	let url = anchor.attr("abs:href")?;
	let key: String = url
		.strip_prefix(&format!("{base_url}/{item_path}/"))?
		.into();
	let cover = element
		.select_first("img")
		.and_then(|img| img.img_attr())
		.or_else(|| Some(guess_cover(base_url, item_path, &key, None)));
	Some(aidoku::Manga {
		key,
		title: anchor.text()?,
		cover,
		..Default::default()
	})
}

pub fn guess_cover(
	base_url: &str,
	item_path: &str,
	manga_slug: &str,
	cover: Option<String>,
) -> String {
	if let Some(cover) = cover
		&& !cover.ends_with("no-image.png")
	{
		return cover;
	}
	format!("{base_url}/uploads/{item_path}/{manga_slug}/cover/cover_250x350.jpg")
}

pub fn chapter_number(title: &str) -> Option<f32> {
	let mut number = String::new();
	let mut started = false;
	let mut dot = false;
	for character in title.chars() {
		if character.is_ascii_digit() {
			number.push(character);
			started = true;
		} else if character == '.' && started && !dot {
			number.push(character);
			dot = true;
		} else if started {
			break;
		}
	}
	number.parse().ok()
}

pub fn status(value: &str) -> MangaStatus {
	match value.to_ascii_lowercase().as_str() {
		"complete" | "complet" | "completo" | "zakończone" | "concluído" | "finalizado"
		| "مكتملة" => MangaStatus::Completed,
		"ongoing" | "مستمرة" | "en cours" | "em lançamento" | "prace w toku" | "ativo"
		| "em andamento" | "activo" => MangaStatus::Ongoing,
		"dropped" => MangaStatus::Cancelled,
		_ => MangaStatus::Unknown,
	}
}

pub fn clean_chapter_name(
	manga_title: &str,
	prefix: &str,
	chapter_string: &str,
	title: &str,
) -> String {
	let initial = title
		.strip_prefix(&format!("{prefix}{manga_title}"))
		.map(|rest| format!("{chapter_string}{rest}"))
		.unwrap_or_else(|| title.into());
	let mut parts = initial.splitn(2, ':').map(str::trim);
	let first = parts.next().unwrap_or_default();
	match parts.next() {
		Some(second) if first != second => format!("{first}: {second}"),
		_ => first.into(),
	}
}

pub fn extract_token(script: &str) -> Option<&str> {
	let marker = "_token";
	let after_marker = script.get(script.find(marker)? + marker.len()..)?;
	let quote = after_marker
		.chars()
		.find(|character| matches!(character, '\'' | '"'))?;
	let value = after_marker.get(after_marker.find(quote)? + quote.len_utf8()..)?;
	value.get(..value.find(quote)?)
}
