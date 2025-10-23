use aidoku::{
	Chapter, Manga, Result,
	alloc::{string::String, vec, vec::Vec},
	imports::net::Request,
	prelude::*,
};

pub fn img_url_handler(url: String) -> String {
	if url.contains(".imgbox.com") {
		url.replace("thumbs", "images")
	} else {
		url
	}
}

pub fn get_manga_json(key: &str) -> Result<serde_json::Value> {
	if key.to_lowercase().starts_with("http") {
		bail!("Unsupported URL");
	}
	let fragments = key.split('/').collect::<Vec<_>>();
	if fragments.len() < 2 {
		bail!("Invalid slug");
	}
	Request::get(format!(
		"https://cubari.moe/read/api/{}/series/{}/",
		fragments[0].to_lowercase(),
		fragments[1]
	))?
	.json_owned()
	.map_err(|_| error!("Invalid slug"))
}

pub fn guide_manga() -> Manga {
	Manga {
		key: "aidoku/guide".into(),
		title: "Cubari Guide".into(),
		cover: Some("https://placehold.co/550x780/ffffff/6e7b91.png?text=Guide".into()),
		chapters: Some(vec![Chapter {
			key: "1".into(),
			title: Some("Guide".into()),
			..Default::default()
		}]),
		..Default::default()
	}
}
