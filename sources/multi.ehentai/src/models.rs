use crate::settings::{TitlePreference, get_base_url, get_title_preference};
use aidoku::{
	ContentRating, Manga, MangaStatus, UpdateStrategy, Viewer,
	alloc::{Vec, string::String},
	prelude::*,
};

#[derive(Debug, Clone)]
pub struct EHTag {
	pub namespace: String,
	pub name: String,
	pub is_weak: bool,
}

#[derive(Debug, Default, Clone)]
pub struct EHGallery {
	pub gid: String,
	pub token: String,
	pub title: String,
	pub alt_title: String,
	pub cover: String,
	pub category: String,
	pub uploader: String,
	pub posted: String,
	pub language: String,
	pub translated: bool,
	pub file_size: String,
	pub length: i32,
	pub favorites: i32,
	pub avg_rating: f64,
	pub rating_count: i32,
	pub visible: String,
	pub tags: Vec<EHTag>,
}

/// Compact gallery info parsed from gallery list pages
#[derive(Debug, Clone)]
pub struct EHGalleryItem {
	pub url: String,
	pub title: String,
	pub alt_title: String,
	pub cover: String,
	pub category: String,
	pub tags: Vec<String>,
	pub language: Option<String>,
}

/// Select display title based on preference, falling back to the other title if needed.
fn select_title(title: String, alt_title: String) -> String {
	let pref = get_title_preference();
	match pref {
		TitlePreference::Japanese if !alt_title.is_empty() => alt_title,
		_ => {
			if title.is_empty() {
				alt_title
			} else {
				title
			}
		}
	}
}

/// Return `Some(v)` if `v` is non-empty, else `None`.
fn non_empty<T: AsRef<[U]>, U>(v: T) -> Option<T> {
	if v.as_ref().is_empty() { None } else { Some(v) }
}

impl From<EHGalleryItem> for Manga {
	fn from(item: EHGalleryItem) -> Self {
		let title = select_title(item.title, item.alt_title);

		let mut authors: Vec<String> = Vec::new();
		let mut groups: Vec<String> = Vec::new();
		let mut parodies: Vec<String> = Vec::new();
		let mut characters: Vec<String> = Vec::new();
		let mut cosplay_tags: Vec<String> = Vec::new();
		let mut other_tags: Vec<String> = Vec::new();
		let mut location_tags: Vec<String> = Vec::new();

		for t in &item.tags {
			if let Some(name) = t.strip_prefix("artist:") {
				authors.push(String::from(name));
			} else if let Some(name) = t.strip_prefix("group:") {
				groups.push(String::from(name));
			} else if let Some(name) = t.strip_prefix("parody:") {
				if name != "original" && name != "various" {
					parodies.push(String::from(name));
				}
			} else if let Some(name) = t.strip_prefix("character:") {
				characters.push(String::from(name));
			} else if let Some(name) = t.strip_prefix("cosplay:") {
				cosplay_tags.push(String::from(name));
			} else if let Some(name) = t.strip_prefix("other:") {
				other_tags.push(String::from(name));
			} else if let Some(name) = t.strip_prefix("location:") {
				location_tags.push(String::from(name));
			}
		}

		// has artist → use artist as authors; no artist → use group as authors
		let use_artist = !authors.is_empty();
		let combined_authors: Vec<String> = if use_artist {
			authors.clone()
		} else {
			groups.clone()
		};

		let other_author_prefix = if use_artist { "group:" } else { "artist:" };
		let tags: Vec<String> = item
			.tags
			.iter()
			.filter(|t| {
				(t.starts_with("female:") || t.starts_with("male:") || t.starts_with("mixed:"))
					|| t.starts_with(other_author_prefix)
			})
			.map(|t| {
				if let Some(pos) = t.find(':') {
					let ns = &t[..pos];
					let rest = &t[pos + 1..];
					let short = if ns == "mixed" {
						'x'
					} else {
						ns.chars().next().unwrap_or('?')
					};
					format!("{}:{}", short, rest)
				} else {
					t.clone()
				}
			})
			.collect();

		let mut desc_parts: Vec<String> = Vec::new();
		if let Some(ref lang) = item.language {
			desc_parts.push(format!("Language: {lang}"));
		}
		// the namespace NOT chosen as authors goes into description
		if use_artist && !groups.is_empty() {
			desc_parts.push(format!("Group: {}", groups.join(", ")));
		} else if !use_artist && !authors.is_empty() {
			desc_parts.push(format!("Artist: {}", authors.join(", ")));
		}
		if !cosplay_tags.is_empty() {
			desc_parts.push(format!("Cosplay: {}", cosplay_tags.join(", ")));
		}
		if !parodies.is_empty() {
			desc_parts.push(format!("Parody: {}", parodies.join(", ")));
		}
		if !characters.is_empty() {
			desc_parts.push(format!("Characters: {}", characters.join(", ")));
		}
		if !other_tags.is_empty() {
			desc_parts.push(format!("Other: {}", other_tags.join(", ")));
		}
		if !location_tags.is_empty() {
			desc_parts.push(format!("Location: {}", location_tags.join(", ")));
		}

		let description = if desc_parts.is_empty() {
			None
		} else {
			Some(desc_parts.join("  \n"))
		};

		Manga {
			key: item.url.clone(),
			title,
			cover: non_empty(item.cover),
			url: Some(item.url),
			description,
			tags: non_empty(tags),
			authors: non_empty(combined_authors),
			content_rating: if item.category == "non-h" {
				ContentRating::Safe
			} else {
				ContentRating::NSFW
			},
			status: MangaStatus::Completed,
			update_strategy: UpdateStrategy::Never,
			..Default::default()
		}
	}
}

impl From<EHGallery> for Manga {
	fn from(gallery: EHGallery) -> Self {
		let title = select_title(gallery.title.clone(), gallery.alt_title.clone());

		let artists: Vec<String> = gallery
			.tags
			.iter()
			.filter(|t| t.namespace == "artist")
			.map(|t| t.name.clone())
			.collect();

		let groups: Vec<String> = gallery
			.tags
			.iter()
			.filter(|t| t.namespace == "group")
			.map(|t| t.name.clone())
			.collect();

		// has artist → use artist as authors; no artist → use group as authors
		let use_artist = !artists.is_empty();
		let combined_authors: Vec<String> = if use_artist {
			artists.clone()
		} else {
			groups.clone()
		};

		let other_author_ns = if use_artist { "group" } else { "artist" };
		let tags: Vec<String> = gallery
			.tags
			.iter()
			.filter(|t| {
				(t.namespace == "female" || t.namespace == "male" || t.namespace == "mixed")
					|| t.namespace == other_author_ns
			})
			.map(|t| {
				let short = if t.namespace == "mixed" {
					'x'
				} else {
					t.namespace.chars().next().unwrap_or('?')
				};
				format!("{}:{}", short, t.name)
			})
			.collect();

		let mut desc_parts: Vec<String> = Vec::new();
		if !gallery.visible.is_empty() && !gallery.visible.eq_ignore_ascii_case("yes") {
			desc_parts.push(format!("Visible: {}", gallery.visible));
		}
		// the namespace NOT chosen as authors goes into description
		if use_artist && !groups.is_empty() {
			desc_parts.push(format!("Group: {}", groups.join(", ")));
		}
		if gallery.length > 0 {
			desc_parts.push(format!("Pages: {}", gallery.length));
		}
		if gallery.avg_rating > 0.0 {
			desc_parts.push(format!(
				"Rating: {:.1} ({} votes)",
				gallery.avg_rating, gallery.rating_count
			));
		}
		if gallery.favorites > 0 {
			desc_parts.push(format!("Favorites: {}", gallery.favorites));
		}
		let cosplay: Vec<String> = gallery
			.tags
			.iter()
			.filter(|t| t.namespace == "cosplay")
			.map(|t| t.name.clone())
			.collect();
		if !cosplay.is_empty() {
			desc_parts.push(format!("Cosplay: {}", cosplay.join(", ")));
		}
		let parodies: Vec<String> = gallery
			.tags
			.iter()
			.filter(|t| t.namespace == "parody" && t.name != "original" && t.name != "various")
			.map(|t| t.name.clone())
			.collect();
		if !parodies.is_empty() {
			desc_parts.push(format!("Parody: {}", parodies.join(", ")));
		}
		let characters: Vec<String> = gallery
			.tags
			.iter()
			.filter(|t| t.namespace == "character")
			.map(|t| t.name.clone())
			.collect();
		if !characters.is_empty() {
			desc_parts.push(format!("Characters: {}", characters.join(", ")));
		}
		let other_ns: Vec<String> = gallery
			.tags
			.iter()
			.filter(|t| t.namespace == "other")
			.map(|t| t.name.clone())
			.collect();
		if !other_ns.is_empty() {
			desc_parts.push(format!("Other: {}", other_ns.join(", ")));
		}
		let locations: Vec<String> = gallery
			.tags
			.iter()
			.filter(|t| t.namespace == "location")
			.map(|t| t.name.clone())
			.collect();
		if !locations.is_empty() {
			desc_parts.push(format!("Location: {}", locations.join(", ")));
		}
		if !gallery.file_size.is_empty() {
			desc_parts.push(format!("File Size: {}", gallery.file_size));
		}
		if !gallery.uploader.is_empty() {
			desc_parts.push(format!("Uploader: {}", gallery.uploader));
		}

		let description = if desc_parts.is_empty() {
			None
		} else {
			Some(desc_parts.join("  \n"))
		};

		let viewer = {
			let cat = gallery.category.to_ascii_lowercase();
			if cat != "manga" && cat != "doujinshi" {
				Viewer::Webtoon
			} else {
				let keywords = [
					"non-h",
					"webtoon",
					"3d",
					"comic",
					"western",
					"screenshots",
					"realporn",
					"artbook",
					"novel",
					"variant set",
					"multipanel sequence",
				];

				let has_webtoon_tag = gallery.tags.iter().any(|t| {
					let ns = t.namespace.to_ascii_lowercase();
					let name = t.name.to_ascii_lowercase();
					ns == "other" && keywords.iter().any(|kw| name.contains(kw))
				});

				if has_webtoon_tag {
					Viewer::Webtoon
				} else if gallery
					.tags
					.iter()
					.any(|t| t.namespace == "language" && t.name == "japanese")
				{
					Viewer::RightToLeft
				} else {
					Viewer::LeftToRight
				}
			}
		};

		let base = get_base_url();
		let url = format!(
			"{}/g/{}/{}/",
			base.trim_end_matches('/'),
			gallery.gid,
			gallery.token
		);

		Manga {
			key: url.clone(),
			title,
			cover: non_empty(gallery.cover),
			description,
			authors: non_empty(combined_authors),
			artists: non_empty(artists),
			url: Some(url),
			tags: non_empty(tags),
			status: MangaStatus::Completed,
			content_rating: if gallery.category == "non-h" {
				ContentRating::Safe
			} else {
				ContentRating::NSFW
			},
			viewer,
			update_strategy: UpdateStrategy::Never,
			..Default::default()
		}
	}
}
