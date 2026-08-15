#![no_std]
use aidoku::{
	Chapter, ContentRating, DeepLinkResult, ImageResponse, Manga, MangaStatus, Page, PageContent,
	PageContext, Result, Source, Viewer,
	alloc::{string::String, vec::Vec},
	canvas::Rect,
	helpers::uri::{decode_uri, encode_uri_component},
	imports::{
		canvas::{Canvas, ImageRef},
		net::Request,
	},
	prelude::*,
};
use wpcomics::{Cache, Impl, Params, WpComics};

mod helpers;
mod models;
#[cfg(test)]
mod test;

use helpers::*;
use models::*;

const IMG_CDN: &str = "https://img-cdn.stackpathcdn.app";

struct SpoilerPlus;

impl Impl for SpoilerPlus {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			viewer: Viewer::RightToLeft,

			manga_cell: "div.items > div.row > article.item > figure.clearfix",
			manga_cell_image_attr: "abs:data-src",
			manga_parse_id: |url| to_key(url).unwrap_or_default(),

			manga_details_title_transformer: clean_title,
			manga_details_cover_attr: "abs:src",
			manga_details_authors_transformer: |authors| {
				authors.into_iter().filter(|a| a != "更新中").collect()
			},
			// genre and tag rows are both li.kind, each value its own anchor
			manga_details_tags: "ul.list-info > li.kind p.col-xs-8 > a",
			manga_details_tags_splitter: "",
			// the alt titles row shares the status row's classes; only the icon differs
			manga_details_status: "ul.list-info > li.row.status:has(i.fa-rss) > p.col-xs-8",
			status_mapping: |status| match status.trim() {
				"連載中" => MangaStatus::Ongoing,
				"完結" | "完了" => MangaStatus::Completed,
				_ => MangaStatus::Unknown,
			},

			// the chapter segment of an href is raw utf-8, and `abs:href` collapses to
			// the page's own url when the host resolver cannot parse it
			chapter_anchor_attr: "href",
			chapter_url_transformer: |href| {
				to_key(&href).map(|key| url_for(&key)).unwrap_or_default()
			},
			// every title is the chapter number, which the app already renders.
			// the template reports an unparsed number as -1.0 rather than as `None`
			chapter_title_transformer: |title, number| match number {
				Some(number) if number >= 0.0 => None,
				_ => title,
			},
			chapter_parse_id: |url| to_key(&url).unwrap_or_default(),

			datetime_format: "yyyy年MM月dd日",
			datetime_locale: "ja_JP",
			datetime_timezone: "Asia/Tokyo",

			manga_page: |_, manga| url_for(&manga.key),
			page_list_page: |_, _, chapter| url_for(&chapter.key),

			get_search_url: |params, query, page, filters| {
				// search has its own endpoint, so a query wins over the sort filter
				if let Some(query) = query {
					let query = encode_uri_component(query);
					return Ok(format!("{}?s={query}&page={page}", params.base_url));
				}

				// no sort parameter: each ordering is served from its own path
				Ok(match sort_index(&filters) {
					1 => format!("{}/ranking/{page}/", params.base_url),
					_ => format!("{}/page/{page}/", params.base_url),
				})
			},

			..Default::default()
		}
	}

	fn category_parser(
		&self,
		params: &Params,
		categories: &Option<Vec<String>>,
	) -> (ContentRating, Viewer) {
		let tags = categories.as_deref().unwrap_or(&[]);
		let rating = if tags
			.iter()
			.any(|tag| tag == "オトナ" || tag.contains("エロ"))
		{
			ContentRating::NSFW
		} else if tags.iter().any(|tag| tag == "Ecchi") {
			ContentRating::Suggestive
		} else {
			ContentRating::Safe
		};
		(rating, params.viewer)
	}

	fn get_page_list(
		&self,
		cache: &mut Cache,
		params: &Params,
		_manga: Manga,
		chapter: Chapter,
	) -> Result<Vec<Page>> {
		let url = url_for(&chapter.key);
		let html = self.create_request(cache, params, &url, None)?.html()?;

		// the reader is empty and pulls the image list from the api, keyed by ids
		// an inline script declares
		// e.g. <script>window.MangaId =  20466 ;window.CNumber =  417 </script>
		let mut manga_id_opt: Option<String> = None;
		let mut chapter_num_opt: Option<String> = None;
		if let Some(scripts) = html.select("script") {
			for script in scripts {
				// the parser leaves script node data empty; read the body as inner html
				if let Some(data) = script.html() {
					if let Some(id) = read_window_number(&data, "window.MangaId", false) {
						manga_id_opt = Some(id);
					}
					if let Some(number) = read_window_number(&data, "window.CNumber", true) {
						chapter_num_opt = Some(number);
					}
					if manga_id_opt.is_some() && chapter_num_opt.is_some() {
						break;
					}
				}
			}
		}
		let manga_id = manga_id_opt.ok_or_else(|| error!("Manga ID not found in {url}"))?;
		let chapter_num =
			chapter_num_opt.ok_or_else(|| error!("Chapter number not found in {url}"))?;

		let api_url = format!("{}/api/v1/get/c", params.base_url);
		let body = format!("{{\"m\":{manga_id},\"n\":{chapter_num}}}");

		let response = Request::post(&api_url)?
			.body(body)
			.header("Content-Type", "application/json")
			.header("Accept", "application/json, text/plain, */*")
			.header("Referer", &url)
			.send()?
			.get_json::<ChapterApiResponse>()?;

		let ChapterApiResponse {
			c: order_key,
			e: paths,
		} = response;

		if paths.is_empty() {
			bail!("No pages found");
		}

		// the descrambling key is per chapter, so every page context gets a copy
		let pages = paths
			.into_iter()
			.map(|path| {
				let img_url = format!("{IMG_CDN}{path}");
				let mut context = PageContext::new();
				context.insert("key".into(), order_key.clone());
				Page {
					content: PageContent::url_context(img_url, context),
					..Default::default()
				}
			})
			.collect::<Vec<_>>();

		Ok(pages)
	}

	fn process_page_image(
		&self,
		_params: &Params,
		response: ImageResponse,
		context: Option<PageContext>,
	) -> Result<ImageRef> {
		let Some(context) = context else {
			return Ok(response.image);
		};
		let Some(order_key) = context.get("key").filter(|s| !s.is_empty()) else {
			return Ok(response.image);
		};

		// pages are scrambled into a square grid, the order handed out as hex bytes
		// xored with the domain
		const XOR_KEY: &str = "spoilerplus.tv";

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
				byte
			})
			.collect::<Vec<u8>>();

		let parts: Vec<i32> = String::from_utf8(decoded_bytes)
			.map_err(|_| error!("Invalid decoded result"))?
			.split(",")
			.filter_map(|s| s.parse().ok())
			.collect();

		let cols = parts.len().isqrt();
		if cols == 0 || cols * cols != parts.len() {
			return Err(error!("Page order is not a square grid"));
		}

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

	fn handle_deep_link(
		&self,
		_cache: &mut Cache,
		params: &Params,
		url: String,
	) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(params.base_url.as_ref()) else {
			return Ok(None);
		};
		// keys are stored decoded, shared links carry the encoded form
		let key = decode_uri(path);

		// series live at the site root, so only the slug suffix separates them from
		// the genre, tag and ranking paths: /TITLE-raw-free/[第N話/]
		const SERIES_SUFFIX: &str = "-raw-free";

		let trimmed = key.trim_end_matches('/');
		let segments: Vec<&str> = trimmed.split('/').filter(|s| !s.is_empty()).collect();

		let Some(series) = segments.first().filter(|s| s.ends_with(SERIES_SUFFIX)) else {
			return Ok(None);
		};
		let manga_key = format!("/{series}/");

		if segments.len() > 1 {
			Ok(Some(DeepLinkResult::Chapter {
				manga_key,
				key: format!("{trimmed}/"),
			}))
		} else {
			Ok(Some(DeepLinkResult::Manga { key: manga_key }))
		}
	}
}

register_source!(
	WpComics<SpoilerPlus>,
	PageImageProcessor,
	ImageRequestProvider,
	DeepLinkHandler
);
