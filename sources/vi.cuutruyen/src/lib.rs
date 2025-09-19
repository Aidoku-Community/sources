// a source made by @c0ntens
#![no_std]
use aidoku::{
	alloc::{string::ToString, vec, String, Vec},
	helpers::uri::QueryParameters,
	imports::{
		canvas::{Canvas, ImageRef, Rect},
		defaults::defaults_get,
		error::AidokuError,
		net::Request,
		std::send_partial_result
	},
	prelude::*,
	BaseUrlProvider, Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageResponse, Listing, ListingProvider,
	Manga, MangaPageResult, Page, PageContent, PageContext, PageImageProcessor, Result, Source
};

mod models;
mod home;

use base64::{engine::general_purpose, Engine};
use models::*;

struct CuuTruyen;

impl Source for CuuTruyen {
	fn new() -> Self { Self }

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let base_url = self.get_base_url()?;
		let mut qs = QueryParameters::new();

		if let Some(query) = query {
			qs.push("q", Some(&query))
		}

		// parse filters
		for filter in filters {
    		match filter {
				// thanks gemini for making this
        		FilterValue::MultiSelect { id: _, included, excluded } => {
                	let mut parts = Vec::new();

                	if !included.is_empty() {
                    	let formatted_tags = included
                    		.iter()
                    		.map(|tag| format!("\"{}\"", tag))
                    		.collect::<Vec<String>>();
                    	parts.push(formatted_tags.join(" and "));
                	}

                	if !excluded.is_empty() {
                    	let formatted_tags = excluded
                        	.iter()
                        	.map(|tag| format!("not \"{}\"", tag))
                        	.collect::<Vec<String>>();
                    	parts.push(formatted_tags.join(" and "));
                	}

					qs.push("tags", Some(&parts.join(" and ")))
        		}
        		_ => return Err(AidokuError::Unimplemented),
    		}
		}

		let url = match qs.is_empty() {
			true => format!("{}/api/v2/mangas/recently_updated?page={}&per_page=30", base_url, page),
			_ => format!("{}/api/v2/mangas/search?{}&page={}&per_page=24", base_url, qs, page),
		};

		let (entries, has_next_page) = Request::get(&url)?
			.send()?
			.get_json::<CuuSearchResponse<Vec<CuuManga>>>()
        	.map(|res| {
        		(
                	res.data
                	.into_iter()
                	.map(|value| value.into_basic_manga())
            		.collect(),
            		res.meta.total_pages.is_some_and(|t| page < t),
        		)
    		})?;

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
		let base_url = self.get_base_url()?;
		let manga_url = format!("{}/api/v2/mangas/{}", base_url, manga.key);
		let manga_res = Request::get(&manga_url)?.send()?.get_json_owned::<CuuSearchResponse<CuuMangaDetails>>()?;
		
		if needs_details {
			manga.copy_from(
				manga_res.clone()
				.data.into(),
			);
			
			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let url = format!("{}/chapters", manga_url);

			manga.chapters = Request::get(&url)?.send()?.get_json::<CuuSearchResponse<Vec<CuuChapter>>>()?
				.data
				.into_iter()
				.map(|chap| {
					let chapter_number = chap.number.parse::<f32>().ok();
					let title = if chapter_number.is_none() && !chap.name.as_ref().is_none_or(|r| r.is_empty()) {
						Some(format!("Ch.{} - {}", chap.number.clone(), chap.name.unwrap().clone()))
					} else if chapter_number.is_none() && chap.name.as_ref().is_none_or(|r| r.is_empty()) {
						Some(format!("Ch.{}", chap.number.clone()))
					} else {
						chap.name.clone()
					};

					Some(Chapter {
						key: chap.id.to_string(),
						title,
						chapter_number,
						date_uploaded: chrono::DateTime::parse_from_rfc3339(&chap.created_at)
							.ok()
							.map(|d| d.timestamp()),
						url: Some(format!("https://truycapcuutruyen.pages.dev/mangas/{}/chapters/{}", manga.key, chap.id)),
						scanlators: manga_res.data.scanlators(),
						..Default::default()
					})
				})
				.collect()
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let base_url = self.get_base_url()?;
		let url = format!("{}/api/v2/chapters/{}", base_url, chapter.key);
		let pages = Request::get(&url)?.send()?.get_json::<CuuSearchResponse<CuuPage>>()?
			.data.pages
			.into_iter()
			.map(|p| {
				let mut context = PageContext::new();
				context.insert(String::from("width"), p.width.unwrap_or(0).to_string());
				context.insert(String::from("height"), p.height.unwrap_or(0).to_string());
				context.insert(String::from("drmData"), p.drm_data.unwrap_or_default().to_string());

				Page {
					content: PageContent::url_context(p.image_url, context),
					..Default::default()
				}
			})
			.collect();

		Ok(pages)
	}
}

impl PageImageProcessor for CuuTruyen {
	fn process_page_image(
		&self,
		response: ImageResponse,
		context: Option<PageContext>,
	) -> Result<ImageRef> {
		let Some(context) = context else {
			return Err(AidokuError::message("Đang bị thiếu trang context. Vui lòng tải lại trang này!!"));
		};

		let width = context
			.get("width")
			.and_then(|w| w.parse::<usize>().ok())
			.unwrap_or(0);
		let height = context
			.get("height")
			.and_then(|h| h.parse::<usize>().ok())
			.unwrap_or(0);
		let drm_data = context
			.get("drmData")
			.map(|drm| drm.replace("\n", ""))
			.unwrap_or_default();

		// if the image is not from the specified CDN, return the original image without trying to descramble
		if response.request.url.is_none() || drm_data.is_empty() {
			return Ok(response.image);
		};

		let drm_key = "3141592653589793";
		let decrypted = general_purpose::STANDARD.decode(&drm_data).unwrap()
			.iter()
			.enumerate()
			.map(|(i, &b)| b ^ drm_key.as_bytes()[i % drm_key.len()])
			.collect::<Vec<u8>>();

		let mut canvas = Canvas::new(width as f32, height as f32);
		let mut sy = 0.0;
		let unscramble = String::from_utf8(decrypted).map_err(|_| AidokuError::Unimplemented)?;
		for segment in unscramble.split("|").skip(1) {
			let part = segment.split('-').collect::<Vec<_>>();
			if part.len() != 2 {
				continue;
			}

			let (dy, part_height) = (part[0].parse::<usize>().unwrap_or(0), part[1].parse::<usize>().unwrap_or(0));
			let src_rect = Rect::new(0.0, sy, width as f32, part_height as f32);
			let des_rect = Rect::new(0.0, dy as f32, width as f32, part_height as f32);

			canvas.copy_image(&response.image, src_rect, des_rect);
			sy += part_height as f32;
		}

		Ok(canvas.get_image())
	}
}

impl ListingProvider for CuuTruyen {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		match listing.id.as_str() {
			"week" | "month" | "all" => {
				let base_url = self.get_base_url()?;
				let (entries, has_next_page) = Request::get(format!("{}/api/v2/mangas/top?duration={}&page={}&per_page=24", base_url, listing.id, page))?
					.send()?
					.get_json::<CuuSearchResponse<Vec<CuuManga>>>()
					.map(|res| {
						(
							res.data
							.into_iter()
							.map(|value| value.into_basic_manga())
							.collect(),
							res.meta.total_pages.is_some_and(|t| page < t)
						)
					})?;

				Ok(MangaPageResult {
					entries,
					has_next_page,
				})
			},
			"latest" => self.get_search_manga_list(None, page, vec![]),
			"vn" => self.get_search_manga_list(None, page, vec![FilterValue::MultiSelect {
				id: String::new(), included: vec!["truyện việt".to_string()], excluded: vec![]
			}]),
			_ => Err(AidokuError::Unimplemented),
		}
	}
}

impl DeepLinkHandler for CuuTruyen {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
    	let all_urls = vec!["https://cuutruyen.net", "https://hetcuutruyen.net", "https://nettrom.com", "https://cuutruyen5c844.site", "https://truycapcuutruyen.pages.dev"];

    	let mut found_base_url = None;
    	for base_url in &all_urls {
    		if url.starts_with(base_url) {
        		found_base_url = Some(url);
            	break;
        	}
    	}

    	let base_url = match found_base_url {
    		Some(url) => url,
    		_ => return Ok(None),
    	};

		let mut results = base_url.split('/').skip(3);
		match results.next() {
			Some("mangas") => match (results.next(), results.next(), results.next()) {
				(Some(key), None, None) => Ok(Some(DeepLinkResult::Manga {
					key: key.into()
				})),
				(Some(manga_key), Some("chapters"), Some(key)) => Ok(Some(DeepLinkResult::Chapter {
					manga_key: manga_key.into(),
					key: key.into()
				})),
				_ => Ok(None),
			}
			_ => Ok(None)
		}
	}
}

impl BaseUrlProvider for CuuTruyen {
	fn get_base_url(&self) -> Result<String> {
		Ok(defaults_get::<String>("url").unwrap_or_default())
	}
}
register_source!(CuuTruyen, Home, ListingProvider, PageImageProcessor, DeepLinkHandler, BaseUrlProvider);
