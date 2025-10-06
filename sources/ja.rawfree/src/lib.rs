#![no_std]
use aidoku::{
	alloc::{
		string::{String, ToString},
		vec::Vec,
	},
	helpers::uri::{encode_uri_component, QueryParameters},
	imports::{
		html::Html,
		net::Request,
		std::{parse_date_with_options, send_partial_result},
	},
	prelude::*,
	AidokuError, Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Manga,
	MangaPageResult, MangaStatus, Page, PageContent, Result, Source, Viewer,
};

mod models;
use models::*;

const BASE_URL: &str = "https://rawfree.me";

struct RawFree;

impl Source for RawFree {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut genre: Option<String> = None;
		for filter in filters {
			if let FilterValue::Select { value, .. } = filter {
				genre = Some(value);
			}
		}

		let url = format!(
			"{BASE_URL}{}/page/{page}/?s={}",
			if let Some(genre) = genre {
				format!("/category/{}", encode_uri_component(genre))
			} else {
				String::new()
			},
			encode_uri_component(query.unwrap_or_default())
		);

		let html = Request::get(&url)?.html()?;

		let entries = html
			.select(".container > .row > div > .entry-ma")
			.map(|elements| {
				elements
					.filter_map(|element| {
						let url = element.select_first("a")?.attr("abs:href")?;
						let key = url.strip_prefix(BASE_URL).map(String::from)?;
						let title = element.select_first("h2")?.attr("title")?;
						let cover = element.select_first("img")?.attr("abs:src");
						Some(Manga {
							key,
							title: clean_title(title),
							cover,
							url: Some(url),
							..Default::default()
						})
					})
					.collect::<Vec<Manga>>()
			})
			.unwrap_or_default();

		let has_next_page = !entries.is_empty(); // html.select_first("nav > a.next").is_some();

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let manga_url = format!("{BASE_URL}{}", manga.key);
		let html = Request::get(&manga_url)?.html()?;

		if needs_details {
			let get_text = |sel: &str| html.select_first(sel).and_then(|e| e.text());

			manga.title = clean_title(get_text("h1.name").unwrap_or(manga.title));
			manga.cover = html
				.select_first(".container .thumb > img")
				.and_then(|el| el.attr("abs:src"));
			manga.description = get_text(".tag-desc > .note-box");
			manga.url = Some(manga_url.clone());
			manga.tags = html
				.select(".genres-wrap > a")
				.map(|els| els.filter_map(|el| el.text()).collect::<Vec<String>>());

			let status_str = get_text(".col span.text-warning").unwrap_or_default();
			manga.status = match status_str.as_str() {
				"OnGoing" => MangaStatus::Ongoing,
				_ => MangaStatus::Unknown,
			};

			let tags = manga.tags.as_deref().unwrap_or(&[]);
			manga.content_rating = if tags
				.iter()
				.any(|e| matches!(e.as_str(), "Ecchi" | "Hentai" | "エロい"))
			{
				ContentRating::NSFW
			} else {
				ContentRating::Safe
			};

			manga.viewer = Viewer::RightToLeft;

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			fn extract_ch_number(s: &str) -> Option<f32> {
				let dai = '第';
				let wa = '話';

				let start = s.find(dai)? + dai.len_utf8();
				let end = s[start..].find(wa)? + start;

				let num_str = &s[start..end];
				num_str.parse().ok()
			}

			manga.chapters = html.select(".entry-chapter").map(|elements| {
				elements
					.filter_map(|element| {
						let link = element.select_first("a")?;
						let url = link.attr("abs:href")?;
						let key = url.strip_prefix(BASE_URL)?.into();
						let chapter_number = extract_ch_number(&link.text()?);
						let date_uploaded = element
							.select_first(".date")
							.and_then(|el| el.text())
							.and_then(|dt| {
								parse_date_with_options(dt, "MMM d, yyyy", "ja_JP", "current")
							});
						Some(Chapter {
							key,
							chapter_number,
							date_uploaded,
							url: Some(url),
							..Default::default()
						})
					})
					.collect::<Vec<_>>()
			});
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{BASE_URL}{}", chapter.key);
		let html = Request::get(url)?.html()?;

		// if there are already page images in the html, we can return them
		let existing_pages = html
			.select(".render > .z_content > img")
			.map(|els| {
				els.filter_map(|el| el.attr("src"))
					.map(|url| Page {
						content: PageContent::url(url),
						..Default::default()
					})
					.collect::<Vec<_>>()
			})
			.unwrap_or_default();
		if !existing_pages.is_empty() {
			return Ok(existing_pages);
		}

		// otherwise, we need to fetch page images from ajax endpoint
		let chapter_js_data = html
			.select_first(".render > script")
			.and_then(|el| el.data())
			.ok_or(error!("failed to find chapter js data"))?;
		let zing_js_data = html
			.select_first("script#zing-dummy-js-header-js-before")
			.and_then(|el| el.data())
			.ok_or(error!("failed to find zing js data"))?;

		fn extract_js_value<'a>(s: &'a str, key: &str) -> Option<&'a str> {
			let key_pattern = format!("{}:", key);
			let start = s.find(&key_pattern)? + key_pattern.len();
			let s = &s[start..].trim_start();

			let value = if let Some(stripped) = s.strip_prefix('\'') {
				// single quote string
				let end = stripped.find('\'')?;
				&stripped[..end]
			} else if let Some(stripped) = s.strip_prefix('"') {
				// double quote string
				let end = stripped.find('"')?;
				&stripped[..end]
			} else {
				// number
				let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
				&s[..end]
			};
			Some(value.trim())
		}

		let p = extract_js_value(&chapter_js_data, "p").ok_or(error!("failed to find p value"))?;
		let chapter_id = extract_js_value(&chapter_js_data, "chapter_id")
			.ok_or(error!("failed to find chapter id"))?;
		let nonce = extract_js_value(&zing_js_data, "\"nonce\"")
			.ok_or(error!("failed to find chapter id"))?;

		let mut content = String::new();
		let mut img_index = 0;

		loop {
			let mut qs = QueryParameters::new();
			qs.push("action", Some("z_do_ajax"));
			qs.push("_action", Some("decode_images"));
			qs.push("p", Some(p));
			qs.push("chapter_id", Some(chapter_id));
			qs.push("img_index", Some(&img_index.to_string()));
			qs.push("content", Some(&content));
			qs.push("nonce", Some(nonce));

			let response = Request::post(format!("{BASE_URL}/wp-admin/admin-ajax.php"))?
				.body(qs.to_string())
				.header(
					"Content-Type",
					"application/x-www-form-urlencoded; charset=utf-8",
				)
				.send()?
				.get_json::<AjaxResponse>()?;

			content.push_str(&response.mes);
			img_index = response.img_index;

			if response.chapter_id.is_some() {
				break;
			}
		}

		let pages = Html::parse_fragment(content)?
			.select("img")
			.map(|els| {
				els.filter_map(|el| {
					let url = el.attr("src")?;
					Some(Page {
						content: PageContent::url(url),
						..Default::default()
					})
				})
				.collect::<Vec<_>>()
			})
			.unwrap_or_default();

		Ok(pages)
	}
}

impl DeepLinkHandler for RawFree {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(key) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};

		const SERIES_PATH: &str = "/manga-raw";
		const CHAPTER_PATH: &str = "/manga-chapter";

		if key.starts_with(SERIES_PATH) {
			// ex: https://rawfree.me/manga-raw/%e6%93%ac%e6%97%8f-raw-free/
			Ok(Some(DeepLinkResult::Manga { key: key.into() }))
		} else if key.starts_with(CHAPTER_PATH) {
			// ex: https://rawfree.me/manga-chapter/%e6%93%ac%e6%97%8f-raw-%e3%80%90%e7%ac%ac12%e8%a9%b1%e3%80%91/
			let html = Request::get(&url)?.html()?;
			let manga_key = html
				.select_first(".manga-name a")
				.and_then(|e| e.attr("href"))
				.and_then(|url| url.strip_prefix(BASE_URL).map(|s| s.into()))
				.ok_or(AidokuError::message("Missing manga key"))?;

			Ok(Some(DeepLinkResult::Chapter {
				manga_key,
				key: key.into(),
			}))
		} else {
			Ok(None)
		}
	}
}

fn clean_title(title: String) -> String {
	let suffixes = ["(Raw – Free)", "(Raw - Free)"];
	for suffix in suffixes {
		if let Some(clean) = title.strip_suffix(suffix) {
			return clean.into();
		}
	}
	title
}

register_source!(RawFree, DeepLinkHandler);
