#![no_std]
use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, ImageResponse, Manga,
	MangaPageResult, MangaStatus, Page, PageContent, PageContext, PageImageProcessor, Result,
	Source, Viewer,
	alloc::{
		Vec,
		string::{String, ToString},
		vec,
	},
	canvas::Rect,
	helpers::uri::QueryParameters,
	imports::{
		canvas::{Canvas, ImageRef},
		net::Request,
		std::{parse_date_with_options, send_partial_result},
	},
	prelude::*,
};
use base64::{Engine, engine::general_purpose::STANDARD};

mod crypto;
mod helpers;

const BASE_URL: &str = "https://www.mangago.me";

struct Mangago;

impl Source for Mangago {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut query = query;

		if let Some(author) = filters.iter().find_map(|filter| match filter {
			FilterValue::Text { value, .. } => Some(value),
			_ => None,
		}) {
			query = Some(author.clone())
		}

		let mut qs = QueryParameters::new();

		let url = if query.is_some() {
			qs.push("name", query.as_deref());
			qs.push("page", Some(&page.to_string()));
			format!("{BASE_URL}/r/l_search?{qs}")
		} else {
			let mut genre = "all".into();
			for filter in filters {
				match filter {
					FilterValue::Sort { id, index, .. } => {
						let value = match index {
							0 => continue,
							1 => "view",
							2 => "comment_count",
							3 => "create_date",
							4 => "update_date",
							_ => continue,
						};
						qs.push(&id, Some(value));
					}
					FilterValue::MultiSelect {
						id,
						included,
						excluded,
					} => match id.as_str() {
						"status" => {
							if !included.contains(&"f".into()) {
								qs.push("f", Some("0"));
							}
							if !included.contains(&"o".into()) {
								qs.push("o", Some("0"));
							}
						}
						"genre" => {
							if !included.is_empty() {
								genre = included.join(",");
							}
							if !excluded.is_empty() {
								qs.push("e", Some(&excluded.join(",")));
							}
						}
						_ => {}
					},
					_ => {}
				}
			}
			format!("{BASE_URL}/genre/{genre}/{page}/?{qs}")
		};
		let html = Request::get(url)?.html()?;

		let entries = html
			.select(".updatesli, .pic_list > li")
			.map(|els| {
				els.filter_map(|el| {
					let link_el = el.select_first(".thm-effect")?;
					let url = link_el.attr("abs:href")?;
					let key = url.strip_prefix(BASE_URL)?.into();
					let title = link_el.attr("title").unwrap_or_default();
					let cover = link_el
						.select_first("img")
						.and_then(|el| el.attr("abs:data-src").or_else(|| el.attr("abs:src")));

					Some(Manga {
						key,
						title,
						cover,
						url: Some(url),
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default();

		Ok(MangaPageResult {
			entries,
			has_next_page: html.select_first(".current + li > a").is_some(),
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{BASE_URL}{}", manga.key);
		let html = Request::get(url)?.html()?;

		if needs_details {
			let Some(info_el) = html.select_first("#information") else {
				bail!("Missing details");
			};
			manga.title = html
				.select_first(".w-title h1")
				.and_then(|el| el.text())
				.unwrap_or(manga.title);
			manga.cover = info_el
				.select_first("img")
				.and_then(|el| el.attr("abs:src"));
			manga.description = info_el
				.select_first(".manga_summary")
				.and_then(|el| el.text());

			for el in info_el
				.select(".manga_info li, .manga_right tr")
				.ok_or_else(|| error!("Select failed"))?
			{
				let Some(title) = el.select_first("b, label").and_then(|el| el.text()) else {
					continue;
				};
				match title.to_ascii_lowercase().as_str() {
					"status:" => {
						manga.status = el
							.select_first("span")
							.and_then(|el| el.text())
							.map(|s| match s.to_ascii_lowercase().as_str() {
								"ongoing" => MangaStatus::Ongoing,
								"completed" => MangaStatus::Completed,
								_ => MangaStatus::Unknown,
							})
							.unwrap_or_default();
					}
					"author(s):" | "author" => {
						manga.authors = el
							.select("a")
							.map(|els| els.filter_map(|el| el.text()).collect());
					}
					"genre(s):" => {
						manga.tags = el
							.select("a")
							.map(|els| els.filter_map(|el| el.text()).collect());

						let tags = manga.tags.as_deref().unwrap_or_default();
						manga.content_rating = if tags
							.as_ref()
							.iter()
							.any(|e| matches!(e.as_str(), "Adult" | "Smut" | "Yaoi"))
						{
							ContentRating::NSFW
						} else if tags.iter().any(|e| e == "Ecchi") {
							ContentRating::Suggestive
						} else {
							ContentRating::Safe
						};
						manga.viewer = if tags.iter().any(|e| e == "Webtoons") {
							Viewer::Webtoon
						} else {
							Viewer::RightToLeft
						};
					}
					_ => continue,
				}
			}

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			manga.chapters = html
				.select("table#chapter_table > tbody > tr, table.uk-table > tbody > tr")
				.map(|els| {
					els.filter_map(|el| {
						let link = el.select_first("a.chico")?;
						let url = link.attr("abs:href")?;
						let key = url.strip_prefix(BASE_URL)?.into();

						let (volume_number, chapter_number, title) =
							helpers::parse_chapter_title(&link.text()?);

						Some(Chapter {
							key,
							volume_number,
							chapter_number,
							title,
							date_uploaded: el
								.select_first("td:last-child")
								.and_then(|el| el.text())
								.and_then(|s| {
									parse_date_with_options(
										s.trim(),
										"MMM d, yyyy",
										"en_US",
										"current",
									)
								}),
							scanlators: el
								.select_first("td.no a, td.uk-table-shrink a")
								.and_then(|el| el.text())
								.map(|s| vec![s]),
							url: Some(url),
							..Default::default()
						})
					})
					.collect()
				});
		}
		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{BASE_URL}{}", chapter.key);
		// https://github.com/keiyoushi/extensions-source/blob/14d648256ec3d2c123da49d619df45fd25c86f36/src/en/mangago/src/eu/kanade/tachiyomi/extension/en/mangago/Mangago.kt#L86
		let html = Request::get(url)?.header("Cookie", "_m_superu=1").html()?;

		let imgsrcs_script = html
			.select("script")
			.ok_or_else(|| error!("Failed to select script elements"))?
			.into_iter()
			.find_map(|el| {
				let data = el.data()?;
				if data.contains("imgsrcs") {
					Some(data)
				} else {
					None
				}
			})
			.ok_or_else(|| error!("Could not find imgsrcs"))?;
		let Some(imgsrcs_raw) = helpers::extract_imgsrcs(&imgsrcs_script) else {
			bail!("Could not extract imgsrcs");
		};
		let imgsrcs = STANDARD
			.decode(imgsrcs_raw)
			.map_err(|e| error!("Base64 decode error: {e}"))?;

		let chapterjs_url = html
			.select("script")
			.ok_or_else(|| error!("Failed to select script elements"))?
			.into_iter()
			.find(|el| el.attr("src").is_some_and(|src| src.contains("chapter.js")))
			.and_then(|el| el.attr("abs:src"))
			.ok_or_else(|| error!("Could not find chapter.js URL"))?;

		let obfuscated_chapter_js = Request::get(chapterjs_url)?.string()?;
		let deobf_chapter_js = helpers::sojson_v4_decode(&obfuscated_chapter_js)?;

		let key = helpers::find_hex_encoded_variable(&deobf_chapter_js, "key")
			.and_then(|key| helpers::decode_hex(key).ok())
			.ok_or_else(|| error!("Could not find cipher key"))?;
		let iv = helpers::find_hex_encoded_variable(&deobf_chapter_js, "iv")
			.and_then(|key| helpers::decode_hex(key).ok())
			.ok_or_else(|| error!("Could not find cipher iv"))?;

		let image_list = crypto::decrypt_key_iv(&imgsrcs, &key, &iv)
			.and_then(|buf| String::from_utf8(buf).ok())
			.map(|s| {
				helpers::unscramble_image_list(
					s.trim_end_matches("\0").trim_end_matches(","),
					&deobf_chapter_js,
				)
			})
			.ok_or_else(|| error!("Failed to decrypt imgsrcs"))?;

		let cols = helpers::find_cols(&deobf_chapter_js).unwrap_or_default();

		Ok(image_list
			.split(",")
			.filter_map(|url| {
				if url.is_empty() {
					return None;
				}
				Some(Page {
					content: if url.contains("cspiclink") {
						let key = match helpers::get_descrambling_key(&deobf_chapter_js, url) {
							Ok(key) => key,
							Err(e) => {
								println!("Failed to get descrambling key: {e:?}");
								return None;
							}
						};
						let mut context = PageContext::new();
						context.insert("desckey".into(), key);
						context.insert("cols".into(), cols.into());
						PageContent::url_context(url, context)
					} else {
						PageContent::url(url)
					},
					..Default::default()
				})
			})
			.collect())
	}
}

impl PageImageProcessor for Mangago {
	fn process_page_image(
		&self,
		response: ImageResponse,
		context: Option<PageContext>,
	) -> Result<ImageRef> {
		let Some(context) = context else {
			return Ok(response.image);
		};
		let (Some(key), Some(cols)) = (
			context.get("desckey"),
			context.get("cols").and_then(|s| s.parse::<i32>().ok()),
		) else {
			return Ok(response.image);
		};

		let image_width = response.image.width();
		let image_height = response.image.height();

		let mut canvas = Canvas::new(image_width, image_height);

		let unit_width = image_width / cols as f32;
		let unit_height = image_height / cols as f32;

		let key_arr: Vec<i32> = key
			.split("a")
			.map(|s| s.parse().unwrap_or_default())
			.collect();

		if key_arr.len() < (cols * cols - 1) as usize {
			bail!("Invalid key array");
		}

		for idx in 0..cols * cols {
			let keyval = key_arr[idx as usize];

			fn floor_div(a: i32, b: i32) -> i32 {
				let (d, r) = (a / b, a % b);
				if (r != 0) && ((r < 0) != (b < 0)) {
					d - 1
				} else {
					d
				}
			}

			let height_y = floor_div(keyval, cols) as f32;
			let dy = height_y * unit_height;
			let dx = (keyval as f32 - height_y * cols as f32) * unit_width;

			let width_y = floor_div(idx, cols) as f32;
			let sy = width_y * unit_height;
			let sx = (idx as f32 - width_y * cols as f32) * unit_width;

			let src_rect = Rect::new(sx, sy, unit_width, unit_height);
			let dst_rect = Rect::new(dx, dy, unit_width, unit_height);

			canvas.copy_image(&response.image, src_rect, dst_rect);
		}

		Ok(canvas.get_image())
	}
}

impl DeepLinkHandler for Mangago {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};

		const READ_MANGA_PATH: &str = "/read-manga";

		if !path.starts_with(READ_MANGA_PATH) {
			return Ok(None);
		}

		let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

		match segments.as_slice() {
			// /read-manga/<manga>/
			["read-manga", manga] => {
				let manga_key = format!("{READ_MANGA_PATH}/{}/", manga);
				Ok(Some(DeepLinkResult::Manga { key: manga_key }))
			}
			// /read-manga/<manga>/mrk/<chapter>/(pg-1/)?
			["read-manga", manga, mrk, chapter, ..] => {
				let manga_key = format!("{READ_MANGA_PATH}/{manga}/",);
				let chapter_key = format!("{READ_MANGA_PATH}/{manga}/{mrk}/{chapter}/");
				Ok(Some(DeepLinkResult::Chapter {
					manga_key,
					key: chapter_key,
				}))
			}
			_ => Ok(None),
		}
	}
}

register_source!(Mangago, PageImageProcessor, DeepLinkHandler);
