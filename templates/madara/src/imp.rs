use super::Params;
use crate::{
	crypto,
	helpers::{self, ElementImageAttr},
	models::*,
};
use aidoku::{
	Chapter, ContentRating, DeepLinkResult, Filter, FilterValue, HomeComponent, HomeLayout, Manga,
	MangaPageResult, MangaStatus, MangaWithChapter, MultiSelectFilter, Page, PageContent,
	PageContext, Result, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	helpers::{element::ElementHelpers, string::StripPrefixOrSelf},
	imports::{
		html::{Document, Element},
		net::Request,
		std::send_partial_result,
	},
	prelude::*,
};
use base64::prelude::*;

pub trait Impl {
	fn new() -> Self;

	fn params(&self) -> Params;

	fn get_search_manga_list(
		&self,
		params: &Params,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let request = if helpers::should_use_load_more(params) {
			helpers::get_search_load_more_request(params, query, page, filters)?
		} else {
			helpers::get_search_request(params, query, page, filters)?
		};
		let html = self.modify_request(params, request)?.html()?;

		helpers::detect_load_more(params, &html);

		Ok(MangaPageResult {
			entries: html
				.select(&params.search_manga_selector)
				.map(|els| {
					els.filter_map(|el| self.parse_manga_element(params, el))
						.collect()
				})
				.unwrap_or_default(),
			has_next_page: html
				.select("div.nav-previous, nav.navigation-ajax, a.nextpostslink")
				.is_some(),
		})
	}

	fn parse_manga_element(&self, params: &Params, element: Element) -> Option<Manga> {
		let url_element = element.select_first(&params.search_manga_url_selector)?;
		let key = url_element
			.attr("abs:href")?
			.strip_prefix_or_self(&params.base_url)
			.into();
		let title_element =
			if params.search_manga_title_selector == params.search_manga_url_selector {
				url_element
			} else {
				element.select_first(&params.search_manga_title_selector)?
			};
		let title = title_element.own_text()?;
		let cover = element
			.select_first(&params.search_manga_cover_selector)?
			.img_attr(params.use_style_images);
		Some(Manga {
			key,
			title,
			cover,
			..Default::default()
		})
	}

	fn get_manga_update(
		&self,
		params: &Params,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{}{}", params.base_url, manga.key);
		let html = self.modify_request(params, Request::get(&url)?)?.html()?;

		if needs_details {
			manga.title = html
				.select_first(&params.details_title_selector)
				.and_then(|e| e.own_text())
				.unwrap_or(manga.title);
			manga.cover = html
				.select_first(&params.details_cover_selector)
				.and_then(|img| img.img_attr(params.use_style_images))
				.or(manga.cover);
			manga.artists = html.select(&params.details_artist_selector).map(|els| {
				els.filter_map(|span| span.text())
					.filter(|t| !t.is_empty())
					.collect()
			});
			manga.authors = html.select(&params.details_author_selector).map(|els| {
				els.filter_map(|span| span.text())
					.filter(|t| !t.is_empty())
					.collect()
			});
			manga.description = html
				.select_first(&params.details_description_selector)
				.and_then(|div| div.text_with_newlines())
				.map(|t| t.trim().into());
			manga.tags = html
				.select(&params.details_tag_selector)
				.map(|els| els.filter_map(|el| el.text()).collect());
			manga.url = Some(url.clone());
			manga.status = html
				.select_first(&params.details_status_selector)
				.and_then(|span| span.text())
				.map(|text| self.get_manga_status(&text))
				.unwrap_or_default();
			manga.content_rating = self.get_manga_content_rating(&html, &manga);
			manga.viewer = html
				.select_first(&params.details_type_selector)
				.and_then(|el| el.own_text())
				.map(|text| self.get_manga_viewer(&text, params.default_viewer))
				.unwrap_or(params.default_viewer);
			send_partial_result(&manga);
		}

		if needs_chapters {
			let mut chapter_elements = html
				.select(&params.chapter_selector)
				.ok_or(error!("Invalid chapter selector"))?;
			if chapter_elements.is_empty() {
				let request = if params.use_new_chapter_endpoint {
					let url = url.strip_suffix("/").unwrap_or(&url);
					Request::post(format!("{url}/ajax/chapters"))?
				} else {
					let manga_id = html
						.select_first("div[id^=manga-chapters-holder]")
						.and_then(|el| el.attr("data-id"))
						.ok_or_else(|| error!("Missing manga ID"))?;
					let body = format!("action=manga_get_chapters&manga={manga_id}");
					Request::post(format!("{}/wp-admin/admin-ajax.php", params.base_url))?
						.body(body)
						.header("Content-Type", "application/x-www-form-urlencoded")
				};
				let html = request
					.header("Referer", &format!("{}/", params.base_url))
					.header("X-Requested-With", "XMLHttpRequest")
					.html()?;
				chapter_elements = html
					.select(&params.chapter_selector)
					.ok_or(error!("Invalid chapter selector"))?;
			}
			manga.chapters = Some(
				chapter_elements
					.filter_map(|el| self.parse_chapter_element(params, el))
					.collect(),
			);
		}

		Ok(manga)
	}

	fn get_manga_status(&self, str: &str) -> MangaStatus {
		match str {
			"OnGoing"
			| "Продолжается"
			| "Updating"
			| "Em Lançamento"
			| "Em lançamento"
			| "Em andamento"
			| "Em Andamento"
			| "En cours"
			| "En Cours"
			| "En cours de publication"
			| "Ativo"
			| "Lançando"
			| "Đang Tiến Hành"
			| "Devam Ediyor"
			| "Devam ediyor"
			| "In Corso"
			| "In Arrivo"
			| "مستمرة"
			| "مستمر"
			| "En Curso"
			| "En curso"
			| "Emision"
			| "Curso"
			| "En marcha"
			| "Publicandose"
			| "Publicándose"
			| "En emision"
			| "连载中"
			| "Devam Ediyo"
			| "Đang làm"
			| "Em postagem"
			| "Devam Eden"
			| "Em progresso"
			| "Em curso"
			| "Atualizações Semanais" => MangaStatus::Ongoing,
			"Completed" | "Completo" | "Completado" | "Concluído" | "Concluido" | "Finalizado"
			| "Achevé" | "Terminé" | "Hoàn Thành" | "مكتملة" | "مكتمل" | "已完结"
			| "Tamamlandı" | "Đã hoàn thành" | "Завершено" | "Tamamlanan" | "Complété" => {
				MangaStatus::Completed
			}
			"On Hold"
			| "Pausado"
			| "En espera"
			| "Durduruldu"
			| "Beklemede"
			| "Đang chờ"
			| "متوقف"
			| "En Pause"
			| "Заморожено"
			| "En attente" => MangaStatus::Hiatus,
			"Canceled" | "Cancelado" | "İptal Edildi" | "Güncel" | "Đã hủy" | "ملغي"
			| "Abandonné" | "Заброшено" | "Annulé" => MangaStatus::Cancelled,
			_ => MangaStatus::Unknown,
		}
	}

	fn get_manga_viewer(&self, str: &str, default: Viewer) -> Viewer {
		match str.to_ascii_lowercase().as_str() {
			"manga" => Viewer::RightToLeft,
			"manhwa" => Viewer::Webtoon,
			"manhua" => Viewer::Webtoon,
			_ => default,
		}
	}

	fn get_manga_content_rating(&self, html: &Document, manga: &Manga) -> ContentRating {
		if html.select_first(".manga-title-badges.adult").is_some() {
			ContentRating::NSFW
		} else if let Some(ref tags) = manga.tags {
			let mut suggestive = false;
			for tag in tags {
				let tag = tag.to_ascii_lowercase();
				if matches!(tag.as_str(), "adult" | "mature") {
					return ContentRating::NSFW;
				} else if matches!(tag.as_str(), "ecchi") {
					suggestive = true;
				}
			}
			if suggestive {
				ContentRating::Suggestive
			} else {
				ContentRating::Safe
			}
		} else {
			ContentRating::Unknown
		}
	}

	fn parse_chapter_element(&self, params: &Params, element: Element) -> Option<Chapter> {
		let url_element = element.select_first(&params.chapter_url_selector)?;
		let url = url_element.attr("abs:href")?;
		let title_element = if params.chapter_title_selector == params.chapter_url_selector {
			url_element
		} else {
			element.select_first(&params.chapter_title_selector)?
		};
		let title_text = title_element.text()?;
		let chapter_number = helpers::find_first_f32(&title_text);

		Some(Chapter {
			key: url.strip_prefix_or_self(&params.base_url).into(),
			title: if title_text.contains("-") {
				title_text
					.split_once('-')
					.map(|(_, title)| title.trim().into())
			} else {
				Some(title_text)
			},
			chapter_number,
			date_uploaded: element
				.select_first(&params.chapter_date_selector)
				.and_then(|time| time.text())
				.map(|date| helpers::parse_chapter_date(params, &date)),
			url: Some(url),
			thumbnail: if !params.chapter_thumbnail_selector.is_empty() {
				element
					.select_first(&params.chapter_thumbnail_selector)
					.and_then(|el| el.img_attr(params.use_style_images))
			} else {
				None
			},
			..Default::default()
		})
	}

	fn get_page_list(&self, params: &Params, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{}{}", params.base_url, chapter.key);
		let html = self.modify_request(params, Request::get(&url)?)?.html()?;

		let Some(chapter_protector) = html.select_first(&params.chapter_protector_selector) else {
			let base_uri = html.select_first("body").unwrap().base_uri().unwrap_or(url);
			let mut context = PageContext::new();
			context.insert("Referer".into(), base_uri);
			return Ok(html
				.select(&params.page_list_selector)
				.map(|els| {
					els.filter_map(|el| {
						let url = el.select_first("img")?.img_attr(params.use_style_images)?;
						Some(Page {
							content: PageContent::url_context(url, context.clone()),
							..Default::default()
						})
					})
					.collect()
				})
				.unwrap_or_default());
		};

		let chapter_protector_html = chapter_protector
			.attr("src")
			.and_then(|src| {
				src.strip_prefix("data:text/javascript;base64,")
					.and_then(|data| BASE64_STANDARD.decode(data).ok())
			})
			.and_then(|vec| String::from_utf8(vec).ok())
			.unwrap_or_else(|| chapter_protector.html().unwrap_or_default());
		let password = helpers::extract_between(
			&chapter_protector_html,
			&params.chapter_protector_password_prefix,
			"';",
		)
		.ok_or(error!("Failed to extract password"))?;
		let data = helpers::extract_between(
			&chapter_protector_html,
			&params.chapter_protector_data_prefix,
			"';",
		)
		.map(|s| s.replace("\\/", "/"))
		.and_then(|s| serde_json::from_str::<ChapterData>(&s).ok())
		.ok_or(error!("Failed to extract data"))?;

		let salted = b"Salted__";
		let unsalted_cipher_text = BASE64_STANDARD
			.decode(data.ct)
			.map_err(|err| error!("Failed to decode base64: {err}"))?;
		let salt = helpers::decode_hex(&data.s).ok_or(error!("Invalid chapter salt"))?;
		let cipher_text = {
			let mut result =
				Vec::with_capacity(salted.len() + salt.len() + unsalted_cipher_text.len());
			result.extend_from_slice(salted);
			result.extend(salt);
			result.extend(unsalted_cipher_text);
			result
		};

		let raw_img_array = crypto::decrypt_key_iv(&cipher_text, password.as_bytes(), None)
			.ok_or(error!("Failed to decrypt chapter data"))?;
		// result is an array of strings, stored in a string (e.g. "[\"1\", \"2\", \"3\"]") with a bunch of extra backslashes
		let img_array = serde_json::from_slice::<String>(&raw_img_array)
			.and_then(|str| serde_json::from_str::<Vec<String>>(&str))
			.map_err(|_| error!("Failed to parse chapter data"))?;

		Ok(img_array
			.into_iter()
			.map(|url| Page {
				content: PageContent::url(url),
				..Default::default()
			})
			.collect())
	}

	fn get_manga_list(
		&self,
		_params: &Params,
		_listing: aidoku::Listing,
		_page: i32,
	) -> Result<MangaPageResult> {
		todo!()
	}

	fn get_home(&self, params: &Params) -> Result<HomeLayout> {
		let html = self
			.modify_request(params, Request::get(&params.base_url)?)?
			.html()?;

		let mut components = Vec::new();

		let parse_manga = |el: &Element| -> Option<Manga> {
			let manga_link = el
				.select_first(".post-title a")
				.or_else(|| el.select_first(".widget-title a"))?;
			Some(Manga {
				key: manga_link
					.attr("href")?
					.strip_prefix_or_self(&params.base_url)
					.into(),
				title: manga_link.text()?,
				cover: el
					.select_first("img")
					.and_then(|img| img.attr("abs:src").or_else(|| img.attr("data-cfsrc"))),
				url: manga_link.attr("href"),
				..Default::default()
			})
		};
		let parse_manga_with_chapter = |el: &Element| -> Option<MangaWithChapter> {
			let manga = parse_manga(el)?;
			let chapter_link = el.select_first(".chapter-item a")?;
			let title_text = chapter_link.text()?;
			let chapter_number = helpers::find_first_f32(&title_text);
			Some(MangaWithChapter {
				manga,
				chapter: Chapter {
					key: chapter_link
						.attr("href")?
						.strip_prefix_or_self(&params.base_url)
						.into(),
					title: if title_text.contains("-") {
						title_text
							.split_once('-')
							.map(|(_, title)| title.trim().into())
					} else {
						Some(title_text)
					},
					chapter_number,
					date_uploaded: el
						.select_first(".timediff a")
						.and_then(|el| el.attr("title"))
						.map(|date| helpers::parse_chapter_date(params, &date)),
					url: chapter_link.attr("href"),
					..Default::default()
				},
			})
		};

		if let Some(popular_slider) = html.select_first(".widget-manga-popular-slider") {
			let title = popular_slider
				.select_first(".heading")
				.and_then(|el| el.text());
			let items = popular_slider
				.select(".slider__item")
				.map(|els| els.filter_map(|el| parse_manga(&el)).collect::<Vec<_>>())
				.unwrap_or_default();
			if !items.is_empty() {
				components.push(HomeComponent {
					title,
					subtitle: None,
					value: aidoku::HomeComponentValue::Scroller {
						entries: items.into_iter().map(|m| m.into()).collect(),
						listing: None,
					},
				});
			}
		}

		if let Some(main_col) = html.select_first(".main-col") {
			let title = main_col
				.select_first(".font-heading .h4")
				.and_then(|el| el.text());
			let last_updates = main_col
				.select(".page-listing-item .manga")
				.map(|els| {
					els.filter_map(|el| parse_manga_with_chapter(&el))
						.collect::<Vec<_>>()
				})
				.unwrap_or_default();
			if !last_updates.is_empty() {
				components.push(HomeComponent {
					title,
					subtitle: None,
					value: aidoku::HomeComponentValue::MangaChapterList {
						page_size: Some(4),
						entries: last_updates,
						listing: None,
					},
				});
			}
		}

		if let Some(recent) = html.select_first(".widget-manga-recent") {
			let title = recent.select_first(".heading").and_then(|el| el.text());
			let todays_trends = recent
				.select(".popular-item-wrap")
				.map(|els| els.filter_map(|el| parse_manga(&el)).collect::<Vec<_>>())
				.unwrap_or_default();
			if !todays_trends.is_empty() {
				components.push(HomeComponent {
					title,
					subtitle: None,
					value: aidoku::HomeComponentValue::Scroller {
						entries: todays_trends.into_iter().map(|m| m.into()).collect(),
						listing: None,
					},
				});
			}
		}

		Ok(HomeLayout { components })
	}

	fn get_dynamic_filters(&self, params: &Params) -> Result<Vec<Filter>> {
		let request = Request::get(format!("{}{}", params.base_url, params.genre_endpoint))?;
		let html = self.modify_request(params, request)?.html()?;

		let (options, ids) = html
			.select_first("div.checkbox-group")
			.ok_or(error!("Failed to find div.checkbox-group"))?
			.select("div.checkbox")
			.ok_or(error!("Failed to select div.checkbox"))?
			.filter_map(|el| {
				let option = el.select_first("label")?.text()?;
				let id = el.select_first("input[type=checkbox]")?.attr("value")?;
				Some((option.into(), id.into()))
			})
			.unzip();

		Ok(vec![
			MultiSelectFilter {
				id: "genre[]".into(),
				title: Some("Genres".into()),
				is_genre: true,
				can_exclude: false,
				options,
				ids: Some(ids),
				..Default::default()
			}
			.into(),
		])
	}

	fn get_image_request(
		&self,
		params: &Params,
		url: String,
		context: Option<PageContext>,
	) -> Result<Request> {
		if let Some(context) = context
			&& let Some(referer) = context.get("Referer")
		{
			return self.modify_request(params, Request::get(url)?.header("Referer", referer));
		}
		self.modify_request(
			params,
			Request::get(url)?.header("Referer", &format!("{}/", params.base_url)),
		)
	}

	fn handle_deep_link(&self, params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(key) = url
			.strip_prefix(params.base_url.as_ref())
			.map(|s| s.to_string())
		else {
			return Ok(None);
		};

		if key.contains("/chapter-") {
			let parts: Vec<&str> = key.split('/').collect();
			if parts.len() < 3 {
				return Ok(None);
			}
			let manga_key = format!("/{}/{}", parts[1], parts[2]);
			Ok(Some(DeepLinkResult::Chapter { manga_key, key }))
		} else {
			Ok(Some(DeepLinkResult::Manga { key }))
		}
	}

	fn handle_id_migration(&self, params: &Params, id: String) -> Result<String> {
		// add source path prefix
		let prefix = format!("/{}/", params.source_path);
		let prefixed_id = if id.starts_with(&prefix) {
			// the id is already in the correct format
			id
		} else {
			format!("{prefix}{id}")
		};
		// add trailing slash
		Ok(if prefixed_id.ends_with('/') {
			prefixed_id
		} else {
			format!("{prefixed_id}/")
		})
	}

	fn modify_request(&self, _params: &Params, request: Request) -> Result<Request> {
		Ok(request)
	}
}
