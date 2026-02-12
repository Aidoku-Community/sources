#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageResponse, Manga, MangaPageResult,
	Page, PageContext, PageImageProcessor, Result, Source,
	alloc::{
		string::{String, ToString},
		vec::Vec,
	},
	helpers::uri::QueryParameters,
	imports::{canvas::ImageRef, defaults::defaults_get, net::Request, std::send_partial_result},
	prelude::*,
};

mod helpers;
mod models;
use models::*;

const BASE_URL: &str = "https://manga.nicovideo.jp";
const API_URL: &str = "https://api.nicomanga.jp/api/v1/app/manga";

const PAGE_SIZE: i32 = 20;

struct NicoVideoSeiga;

impl Source for NicoVideoSeiga {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut params = QueryParameters::new();
		params.push("mode", Some("keyword"));
		params.push("q", query.as_deref());
		params.push("limit", Some(&PAGE_SIZE.to_string()));
		params.push("offset", Some(&((page - 1) * PAGE_SIZE).to_string()));

		params.push("sort", Some("score"));

		for filter in filters {
			if let FilterValue::Sort {
				id,
				index,
				ascending,
			} = filter
			{
				let value = match index {
					0 => "score",
					1 => "contents_updated",
					2 => "favorite_count",
					3 => "comment_created",
					4 => "view_count",
					5 => "contents_created",
					6 => "comment_count",
					_ => "score",
				};
				params.set(
					&id,
					Some(&format!("{}{value}", if ascending { "-" } else { "" })),
				);
			}
		}

		let url = format!("{API_URL}/contents?{params}");
		Request::get(url)?
			.json_owned::<ApiResponse<Vec<NicoManga>>>()
			.map(Into::into)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			manga.copy_from(
				Request::get(format!("{API_URL}/contents/{}", manga.key))?
					.json_owned::<ApiResponse<NicoManga>>()?
					.into(),
			);
			if needs_chapters {
				send_partial_result(&manga);
			}
		}
		if needs_chapters {
			let skip_locked = !defaults_get::<bool>("showLocked").unwrap_or(true);
			manga.chapters = Some(
				Request::get(format!(
					"{API_URL}/contents/{}/episodes?sort=episode_number",
					manga.key
				))?
				.json_owned::<ApiResponse<Vec<NicoChapter>>>()?
				.data
				.result
				.into_iter()
				.filter(|c| c.own_status.sell_status != "publication_finished")
				.filter(|c| {
					if skip_locked {
						c.own_status.sell_status != "selling"
					} else {
						true
					}
				})
				.map(Into::into)
				.collect(),
			);
		}
		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{API_URL}/episodes/{}/frames?enable_webp=true", chapter.key);
		Request::get(url)?
			.json_owned::<ApiResponse<Vec<NicoFrame>>>()
			.map(|res| res.data.result.into_iter().map(Into::into).collect())
	}
}

impl PageImageProcessor for NicoVideoSeiga {
	fn process_page_image(
		&self,
		response: ImageResponse,
		context: Option<PageContext>,
	) -> Result<ImageRef> {
		let Some(key) = context.as_ref().and_then(|c| c.get("drm_hash")) else {
			return Ok(response.image);
		};

		// https://github.com/keiyoushi/extensions-source/blob/8f70beda06a70f84c79d793367fbdf6b9ea09b5a/src/ja/nicovideoseiga/src/eu/kanade/tachiyomi/extension/ja/nicovideoseiga/NicovideoSeiga.kt#L253
		fn decrypt_image(key: &str, image: &mut [u8]) {
			let mut key_set = [0u8; 8];
			for i in 0..8 {
				let hex_str = &key[2 * i..2 * i + 2];
				key_set[i] = u8::from_str_radix(hex_str, 16).unwrap_or(0);
			}
			for (i, byte) in image.iter_mut().enumerate() {
				*byte ^= key_set[i % 8];
			}
		}

		let mut data = response.image.data();
		decrypt_image(key, &mut data);

		Ok(ImageRef::new(&data))
	}
}

impl DeepLinkHandler for NicoVideoSeiga {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		// https://manga.nicovideo.jp/comic/XXXXX
		// https://sp.manga.nicovideo.jp/comic/XXXXX
		// https://manga.nicovideo.jp/watch/mgXXXXXX

		let Some(path_start) = url.find(".jp/") else {
			return Ok(None);
		};
		let path = &url[path_start + 3..];

		const COMIC_PATH: &str = "/comic/";

		if let Some(rest) = path.strip_prefix(COMIC_PATH) {
			let end = rest.find('/').unwrap_or(rest.len());
			let key = &rest[..end];
			Ok(Some(DeepLinkResult::Manga { key: key.into() }))
		} else {
			Ok(None)
		}
	}
}

register_source!(NicoVideoSeiga, PageImageProcessor, DeepLinkHandler);
