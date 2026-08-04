use super::Params;
use crate::{
	helpers::{self, ElementImageAttr},
	models::SearchResult,
};
use aidoku::{
	Chapter, DeepLinkResult, Filter, FilterValue, HomeLayout, Manga, MangaPageResult,
	MultiSelectFilter, Page, PageContent, PageContext, Result,
	alloc::{String, Vec, borrow::Cow, format, string::ToString, vec},
	helpers::uri::{QueryParameters, encode_uri_component},
	imports::{
		html::{Document, Element},
		net::Request,
		std::{parse_date, send_partial_result},
	},
};

pub trait Impl {
	fn new() -> Self;

	fn params(&self) -> Params;

	fn modify_request(&self, _params: &Params, request: Request) -> Result<Request> {
		Ok(request)
	}

	fn parse_manga_element(&self, params: &Params, element: &Element) -> Option<Manga> {
		helpers::manga_from_element(element, &params.base_url, &params.item_path)
	}

	fn parse_manga_page(&self, params: &Params, html: &Document) -> MangaPageResult {
		MangaPageResult {
			entries: html
				.select(&params.manga_list_selector)
				.map(|els| {
					els.filter_map(|element| self.parse_manga_element(params, &element))
						.collect()
				})
				.unwrap_or_default(),
			has_next_page: html.select(&params.manga_list_next_page_selector).is_some(),
		}
	}

	fn parse_manga_details(&self, params: &Params, html: &Document, manga: &mut Manga, url: &str) {
		manga.title = html
			.select_first(&params.details_title_selector)
			.and_then(|el| el.text())
			.unwrap_or_else(|| manga.title.clone());
		manga.cover = html
			.select_first(".row img.img-responsive")
			.and_then(|img| img.img_attr())
			.map(|cover| {
				helpers::guess_cover(&params.base_url, &params.item_path, &manga.key, Some(cover))
			})
			.or_else(|| manga.cover.clone());
		manga.description = html
			.select(".row .well")
			.and_then(|mut els| els.next())
			.and_then(|el| el.text());
		manga.url = Some(url.into());
		if let (Some(labels), Some(values)) = (
			html.select(".row .dl-horizontal dt"),
			html.select(".row .dl-horizontal dt + dd"),
		) {
			for (label, value) in labels.zip(values) {
				let label = label
					.text()
					.unwrap_or_default()
					.trim_end_matches(':')
					.to_ascii_lowercase();
				let value_text = value.text().unwrap_or_default();
				match label.as_str() {
					"author(s)" | "autor(es)" | "auteur(s)" | "著作" | "yazar(lar)"
					| "mangaka(lar)" | "pengarang/penulis" | "pengarang" | "penulis" | "autor"
					| "المؤلف" | "перевод" | "autor/autorzy" => {
						manga.authors = Some(vec![value_text])
					}
					"artist(s)"
					| "artiste(s)"
					| "sanatçi(lar)"
					| "artista(s)"
					| "artist(s)/ilustrator"
					| "الرسام"
					| "seniman"
					| "rysownik/rysownicy"
					| "artista" => manga.artists = Some(vec![value_text]),
					"categories" | "categorías" | "catégories" | "ジャンル" | "kategoriler"
					| "categorias" | "kategorie" | "التصنيفات" | "жанр" | "kategori" | "tagi"
					| "género" => {
						manga.tags = value
							.select("a")
							.map(|els| els.filter_map(|tag| tag.text()).collect())
					}
					"status" => manga.status = helpers::status(&value_text),
					_ => {}
				}
			}
		}
	}

	fn parse_chapter_element(
		&self,
		params: &Params,
		element: &Element,
		manga: &Manga,
	) -> Option<Chapter> {
		let title_wrapper = element.select_first(".chapter-title-rtl")?;
		let anchor = title_wrapper.select_first("a")?;
		let url = anchor.attr("abs:href")?;
		let title = helpers::clean_chapter_name(
			&manga.title,
			&params.chapter_name_prefix,
			&params.chapter_string,
			&title_wrapper.text()?,
		);
		Some(Chapter {
			key: url
				.strip_prefix(&format!(
					"{}/{}/{}/",
					params.base_url, params.item_path, manga.key
				))?
				.into(),
			chapter_number: helpers::chapter_number(&title),
			title: Some(title),
			date_uploaded: element
				.select_first(".date-chapter-title-rtl")
				.and_then(|el| el.text())
				.and_then(|date| parse_date(&date, &params.date_format)),
			url: Some(url),
			..Default::default()
		})
	}

	fn parse_page_list(&self, _params: &Params, html: &Document) -> Vec<Page> {
		html.select("#all > img.img-responsive")
			.map(|els| {
				els.filter_map(|image| {
					Some(Page {
						content: PageContent::url(image.img_attr()?),
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default()
	}

	fn get_search_directory(
		&self,
		params: &Params,
		query: &str,
		page: i32,
	) -> Result<MangaPageResult> {
		let url = format!(
			"{}/search?query={}",
			params.base_url,
			encode_uri_component(query)
		);
		let results = self
			.modify_request(params, Request::get(url)?)?
			.json_owned::<SearchResult>()?;
		let start = ((page - 1).max(0) as usize) * 24;
		let entries = results
			.suggestions
			.into_iter()
			.skip(start)
			.take(24)
			.map(|suggestion| {
				let cover = suggestion.cover.unwrap_or_else(|| {
					helpers::guess_cover(
						&params.base_url,
						&params.item_path,
						&suggestion.data,
						None,
					)
				});
				Manga {
					key: suggestion.data,
					title: suggestion.value,
					cover: Some(cover),
					..Default::default()
				}
			})
			.collect::<Vec<_>>();
		Ok(MangaPageResult {
			has_next_page: entries.len() == 24,
			entries,
		})
	}

	fn get_search_manga_list(
		&self,
		params: &Params,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		if let Some(query) = query {
			return self.get_search_directory(params, &query, page);
		}

		let mut qs = QueryParameters::new();
		qs.push("page", Some(&page.to_string()));
		for filter in filters {
			match filter {
				FilterValue::Text { id, value } => qs.push(&id, Some(&value)),
				FilterValue::Select { id, value } => qs.push(&id, Some(&value)),
				FilterValue::Sort {
					id,
					index,
					ascending,
				} => {
					qs.push(&id, Some(&index.to_string()));
					qs.push("asc", Some(&ascending.to_string()));
				}
				FilterValue::MultiSelect { id, included, .. } => {
					for value in included {
						qs.push(&id, Some(&value));
					}
				}
				_ => {}
			}
		}

		if params.supports_advanced_search {
			let filters_page = self
				.modify_request(
					params,
					Request::get(format!("{}/advanced-search", params.base_url))?
						.header("Referer", &format!("{}/", params.base_url)),
				)?
				.html()?;
			let token = filters_page.select("script").and_then(|scripts| {
				scripts
					.filter_map(|script| script.data())
					.find_map(|data| helpers::extract_token(&data).map(String::from))
			});
			let mut body = format!(
				"params={}&page={page}",
				encode_uri_component(qs.to_string())
			);
			if let Some(token) = token {
				body.push_str(&format!("&_token={}", encode_uri_component(&token)));
			}
			let html = self
				.modify_request(
					params,
					Request::post(format!("{}/advSearchFilter", params.base_url,))?
						.header("Content-Type", "application/x-www-form-urlencoded")
						.header("Referer", &format!("{}/advanced-search", params.base_url))
						.body(body),
				)?
				.html()?;
			return Ok(self.parse_manga_page(params, &html));
		}

		let url = format!("{}/filterList?{qs}", params.base_url);
		let html = self
			.modify_request(
				params,
				Request::get(url)?.header("Referer", &format!("{}/", params.base_url)),
			)?
			.html()?;
		Ok(self.parse_manga_page(params, &html))
	}

	fn get_manga_list(
		&self,
		params: &Params,
		listing: aidoku::Listing,
		page: i32,
	) -> Result<MangaPageResult> {
		let url = match listing.id.as_str() {
			"popular" => format!(
				"{}/filterList?page={page}&sortBy=views&asc=false",
				params.base_url
			),
			_ => format!("{}/latest-release?page={page}", params.base_url),
		};
		let html = self
			.modify_request(
				params,
				Request::get(url)?.header("Referer", &format!("{}/", params.base_url)),
			)?
			.html()?;
		Ok(self.parse_manga_page(params, &html))
	}

	fn get_manga_update(
		&self,
		params: &Params,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{}/{}/{}", params.base_url, params.item_path, manga.key);
		let html = self
			.modify_request(
				params,
				Request::get(&url)?.header("Referer", &format!("{}/", params.base_url)),
			)?
			.html()?;

		if needs_details {
			self.parse_manga_details(params, &html, &mut manga, &url);
			send_partial_result(&manga);
		}

		if needs_chapters {
			manga.chapters = html.select(&params.chapter_list_selector).map(|els| {
				els.filter_map(|el| self.parse_chapter_element(params, &el, &manga))
					.collect()
			});
		}
		Ok(manga)
	}

	fn get_page_list(&self, params: &Params, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let html = self
			.modify_request(
				params,
				Request::get(format!(
					"{}/{}/{}/{}",
					params.base_url, params.item_path, manga.key, chapter.key
				))?
				.header("Referer", &format!("{}/", params.base_url)),
			)?
			.html()?;
		Ok(self.parse_page_list(params, &html))
	}

	fn get_dynamic_filters(&self, params: &Params) -> Result<Vec<Filter>> {
		if !params.supports_advanced_search {
			return Ok(Vec::new());
		}
		let html = self
			.modify_request(
				params,
				Request::get(format!("{}/advanced-search", params.base_url))?,
			)?
			.html()?;
		let mut filters = Vec::new();
		for (name, title, genre) in [
			("categories[]", "Categories", true),
			("status[]", "Status", false),
			("types[]", "Types", false),
		] {
			let selector = format!("select[name='{name}'] option");
			let Some(options) = html.select(&selector) else {
				continue;
			};
			let (labels, ids): (Vec<_>, Vec<_>) = options
				.filter_map(|option| {
					Some((
						Cow::Owned(option.text()?),
						Cow::Owned(option.attr("value")?),
					))
				})
				.unzip();
			if !labels.is_empty() {
				filters.push(
					MultiSelectFilter {
						id: name.into(),
						title: Some(title.into()),
						is_genre: genre,
						can_exclude: false,
						options: labels,
						ids: Some(ids),
						..Default::default()
					}
					.into(),
				);
			}
		}
		Ok(filters)
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

	fn get_home(&self, _params: &Params) -> Result<HomeLayout> {
		Err(aidoku::AidokuError::Unimplemented)
	}

	fn handle_deep_link(&self, params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(key) = url.strip_prefix(&format!("{}/{}/", params.base_url, params.item_path))
		else {
			return Ok(None);
		};
		let slash_count = key.matches('/').count();
		if slash_count > 1 || (slash_count == 1 && !key.ends_with('/')) {
			let mut components = key.split('/');
			let manga_key = components.next().unwrap_or_default().into();
			let chapter_key = components.next().unwrap_or_default().into();
			Ok(Some(DeepLinkResult::Chapter {
				manga_key,
				key: chapter_key,
			}))
		} else {
			Ok(Some(DeepLinkResult::Manga {
				key: key.trim_end_matches("/").into(),
			}))
		}
	}
}
