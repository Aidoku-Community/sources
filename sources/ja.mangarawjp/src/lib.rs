#![no_std]
use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider,
	ImageResponse, Manga, MangaPageResult, Page, PageContent, PageContext, PageImageProcessor,
	Result, Source, Viewer,
	alloc::{string::String, vec::Vec},
	canvas::Rect,
	helpers::uri::encode_uri_component,
	imports::{
		canvas::{Canvas, ImageRef},
		net::Request,
		std::send_partial_result,
	},
	prelude::*,
};

mod helpers;
mod models;

use helpers::*;
use models::*;

const BASE_URL: &str = "https://mangarawjp.tv";
const IMG_CDN: &str = "https://img-cdn.stackpathcdn.app";

struct MangaRawJP;

impl Source for MangaRawJP {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let url = format!(
			"{BASE_URL}?s={}&page={page}",
			encode_uri_component(query.unwrap_or_default())
		);

		let html = Request::get(&url)?.html()?;

		let entries = html
			.select(".post-list > a")
			.map(|elements| {
				elements
					.filter_map(|element| {
						let url = element.attr("abs:href")?;
						let key = url.strip_prefix(BASE_URL).map(String::from)?;
						let title = element.select_first("h3")?.text()?;
						let cover = element.select_first("img").and_then(|img| {
							img.attr("abs:data-src").or_else(|| img.attr("abs:src"))
						});
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

		let has_next_page = !entries.is_empty();

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
			manga.title = clean_title(
				html.select_first("h1")
					.and_then(|e| e.text())
					.unwrap_or(manga.title),
			);
			manga.cover = html
				.select_first(".post-cover > img")
				.and_then(|el| el.attr("abs:src"));
			manga.description = html.select(".page-h p").and_then(|els| {
				let texts: Vec<String> = els.filter_map(|el| el.own_text()).collect();
				if texts.is_empty() {
					None
				} else {
					Some(texts.join("\n "))
				}
			});
			manga.url = Some(manga_url);
			manga.tags = html.select(".category-warp > a, .tag-list > a").map(|els| {
				let mut tags = els.filter_map(|el| el.text()).collect::<Vec<String>>();
				tags.sort();
				tags.dedup();
				tags
			});
			let tags = manga.tags.as_deref().unwrap_or(&[]);
			manga.content_rating = if tags.iter().any(|e| e == "オトナ" || e.contains("エロ"))
			{
				ContentRating::NSFW
			} else if tags.iter().any(|e| e == "Ecchi") {
				ContentRating::Suggestive
			} else {
				ContentRating::Safe
			};
			manga.viewer = Viewer::RightToLeft;

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			manga.chapters = html.select(".ch-list li a").map(|elements| {
				elements
					.filter_map(|element| {
						let url = element.attr("abs:href")?;
						let key = url.strip_prefix(BASE_URL)?.into();
						let title_text = element.text()?;
						let chapter_number = extract_ch_number(&title_text);
						Some(Chapter {
							key,
							chapter_number,
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
		let html = Request::get(&url)?.html()?;

		// Extract window.MangaId and window.CNumber from inline script tags
		// e.g. <script>window.MangaId =  133 ;window.CNumber =  10 </script>
		let mut manga_id_opt: Option<String> = None;
		let mut chapter_num_opt: Option<String> = None;
		if let Some(scripts) = html.select("script") {
			for script in scripts {
				if let Some(data) = script.data() {
					// Look for window.MangaId
					if let Some(pos) = data.find("window.MangaId") {
						let after = &data[pos + 14..]; // after "window.MangaId"
						if let Some(eq_pos) = after.find('=') {
							let after_eq = after[eq_pos + 1..].trim_start();
							let end = after_eq
								.find(|c: char| !c.is_ascii_digit())
								.unwrap_or(after_eq.len());
							let num_str = after_eq[..end].trim();
							if !num_str.is_empty() {
								manga_id_opt = Some(num_str.into());
							}
						}
					}
					// Look for window.CNumber
					if let Some(pos) = data.find("window.CNumber") {
						let after = &data[pos + 14..]; // after "window.CNumber"
						if let Some(eq_pos) = after.find('=') {
							let after_eq = after[eq_pos + 1..].trim_start();
							let end = after_eq
								.find(|c: char| !c.is_ascii_digit() && c != '.')
								.unwrap_or(after_eq.len());
							let num_str = after_eq[..end].trim();
							if !num_str.is_empty() {
								chapter_num_opt = Some(num_str.into());
							}
						}
					}
					if manga_id_opt.is_some() && chapter_num_opt.is_some() {
						break;
					}
				}
			}
		}
		let manga_id = manga_id_opt.ok_or_else(|| error!("Manga ID not found"))?;
		let chapter_num = chapter_num_opt.ok_or_else(|| error!("Chapter number not found"))?;

		// Fetch image URL list via JSON API
		let api_url = format!("{BASE_URL}/api/v1/get/c");
		let body = format!("{{\"m\":{manga_id},\"n\":{chapter_num}}}");

		let response = Request::post(&api_url)?
			.body(body)
			.header("Content-Type", "application/json")
			.header("Accept", "application/json, text/plain, */*")
			.header("Referer", &url)
			.send()?
			.get_json::<ChapterApiResponse>()?;

		let pages = response
			.e
			.into_iter()
			.map(|path| {
				let img_url = format!("{IMG_CDN}{path}");
				let mut context = PageContext::new();
				context.insert("key".into(), response.c.clone());
				Page {
					content: PageContent::url_context(img_url, context),
					..Default::default()
				}
			})
			.collect::<Vec<_>>();

		Ok(pages)
	}
}

impl PageImageProcessor for MangaRawJP {
	fn process_page_image(
		&self,
		response: ImageResponse,
		context: Option<PageContext>,
	) -> Result<ImageRef> {
		let Some(context) = context else {
			return Ok(response.image);
		};
		let Some(order_key) = context
			.get("key")
			.and_then(|s| if s.is_empty() { None } else { Some(s) })
		else {
			return Ok(response.image);
		};

		const XOR_KEY: &str = "mangarawjp.tv";

		let order_bytes = order_key
			.as_bytes()
			.chunks(2)
			.map(|chunk| {
				core::str::from_utf8(chunk)
					.ok()
					.and_then(|hex| u8::from_str_radix(hex, 16).ok())
					.ok_or_else(|| error!("Invalid order key"))
			})
			.collect::<Result<Vec<u8>>>()?;

		let key_bytes = XOR_KEY.as_bytes();
		let decoded_bytes = order_bytes
			.into_iter()
			.map(|mut byte| {
				for &k in key_bytes {
					byte ^= k;
				}
				Ok(byte)
			})
			.collect::<Result<Vec<u8>>>()?;

		let parts: Vec<i32> = String::from_utf8(decoded_bytes)
			.map_err(|_| error!("Invalid decoded result"))?
			.split(",")
			.filter_map(|s| s.parse().ok())
			.collect();

		let cols = parts.len().isqrt();

		let image_width = response.image.width();
		let image_height = response.image.height();

		let mut canvas = Canvas::new(image_width, image_height);

		let unit_width = image_width / cols as f32;
		let unit_height = image_height / cols as f32;

		for (i, pos) in parts.iter().enumerate() {
			let sx = (*pos % cols as i32) as f32 * unit_width;
			let sy = (*pos / cols as i32) as f32 * unit_height;

			let dx = (i % cols) as f32 * unit_width;
			let dy = (i / cols) as f32 * unit_height;

			let src_rect = Rect::new(sx, sy, unit_width, unit_height);
			let dst_rect = Rect::new(dx, dy, unit_width, unit_height);

			canvas.copy_image(&response.image, src_rect, dst_rect);
		}

		Ok(canvas.get_image())
	}
}

impl ImageRequestProvider for MangaRawJP {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", &format!("{BASE_URL}/")))
	}
}

impl DeepLinkHandler for MangaRawJP {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(key) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};

		const SERIES_PATH: &str = "/manga-raw/";

		if key.starts_with(SERIES_PATH) {
			// Determine chapter vs series URL by path segment count
			// Series:  /manga-raw/TITLE-raw-free/
			// Chapter: /manga-raw/TITLE-raw-free/第N話/
			let trimmed = key.trim_end_matches('/');
			let segments: Vec<&str> = trimmed.split('/').filter(|s| !s.is_empty()).collect();

			if segments.len() > 2 {
				// Chapter URL — derive manga key from parent path
				let manga_key = segments[..2].join("/");
				let manga_key = format!("/{manga_key}/");
				Ok(Some(DeepLinkResult::Chapter {
					manga_key,
					key: key.into(),
				}))
			} else {
				// Series URL
				Ok(Some(DeepLinkResult::Manga { key: key.into() }))
			}
		} else {
			Ok(None)
		}
	}
}

register_source!(
	MangaRawJP,
	PageImageProcessor,
	ImageRequestProvider,
	DeepLinkHandler
);
