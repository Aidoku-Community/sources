use super::Params;
use aidoku::{
	Chapter, ContentRating, DeepLinkResult, FilterValue, HomeComponent, HomeComponentValue,
	HomeLayout, Link, Listing, Manga, MangaPageResult, MangaStatus, MangaWithChapter, Page,
	PageContent, PageContext, Result, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	helpers::{string::StripPrefixOrSelf, uri::QueryParameters},
	imports::{
		net::Request,
		std::{current_date, parse_date_with_options, send_partial_result},
	},
	prelude::*,
};

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
		let mut qs = QueryParameters::new();
		let mut min_chapters = None;
		let mut max_chapters = None;
		qs.push("page", Some(&page.to_string()));
		if query.is_some() {
			qs.push("keyword", query.as_deref());
		}

		for filter in filters {
			match filter {
				FilterValue::Sort { id, index, .. } => {
					let value = match index {
						0 => "most_read",
						1 => "less_read",
						2 => "newest",
						3 => "oldest",
						4 => "a-z",
						5 => "z-a",
						6 => "top_rated",
						_ => "",
					};
					if !value.is_empty() {
						qs.push(&id, Some(value));
					}
				}
				FilterValue::Text { id, value } if id == "min_chapters" || id == "max_chapters" => {
					if !value.trim().is_empty() {
						let count = value.trim().parse::<usize>().map_err(|_| {
							error!("Inserisci un numero di capitoli intero e positivo")
						})?;
						if id == "min_chapters" {
							min_chapters = Some(count);
						} else {
							max_chapters = Some(count);
						}
					}
				}
				FilterValue::Text { id, value } => {
					if !value.trim().is_empty() {
						qs.set(&id, Some(&value));
					}
				}
				FilterValue::Select { id, value } => {
					qs.set(&id, Some(&value));
				}
				FilterValue::MultiSelect { id, included, .. } => {
					for item in included {
						qs.push(&id, Some(&item));
					}
				}
				_ => {}
			}
		}

		if matches!((min_chapters, max_chapters), (Some(min), Some(max)) if min > max) {
			return Err(error!("Il minimo non può superare il massimo"));
		}
		let url = format!("{}/archive?{qs}", params.base_url);
		let mut result = self.parse_manga_page(params, url)?;
		if min_chapters.is_none() && max_chapters.is_none() {
			return Ok(result);
		}
		let mut filtered = Vec::new();
		for manga in result.entries {
			let details = self.get_manga_update(params, manga, false, true)?;
			let count = details
				.chapters
				.as_ref()
				.ok_or(error!("Impossibile determinare il numero dei capitoli"))?
				.len();
			if min_chapters.is_none_or(|min| count >= min)
				&& max_chapters.is_none_or(|max| count <= max)
			{
				filtered.push(details);
			}
		}
		result.entries = filtered;
		Ok(result)
	}

	fn get_manga_list(
		&self,
		params: &Params,
		listing: Listing,
		page: i32,
	) -> Result<MangaPageResult> {
		let path = match listing.id.as_str() {
			"latest" => format!("/?page={page}"),
			"new" => format!("/archive?sort=newest&page={page}"),
			"popular" => format!("/archive?sort=most_read&page={page}"),
			"rated" => format!("/archive?sort=top_rated&page={page}"),
			_ => return Err(error!("Unknown listing")),
		};
		self.parse_manga_page(params, format!("{}{path}", params.base_url))
	}

	fn get_home(&self, params: &Params) -> Result<HomeLayout> {
		let mut components = Vec::new();
		for (id, title) in [
			("latest", "Ultimi aggiornamenti"),
			("new", "Nuovi manga"),
			("popular", "Più letti"),
			("rated", "Più votati"),
		] {
			let listing = Listing {
				id: id.into(),
				name: title.into(),
				..Default::default()
			};
			let result = match self.get_manga_list(params, listing.clone(), 1) {
				Ok(result) => result,
				Err(err) if id == "latest" => return Err(err),
				Err(_) => continue,
			};
			if result.entries.is_empty() {
				if id == "latest" {
					return Err(error!("No latest updates found"));
				}
				continue;
			}
			let value = if id == "latest" {
				// Keep the site's order, and limit extra detail requests to the first 12 titles.
				let mut entries = Vec::new();
				for manga in result.entries.iter().take(12) {
					if let Ok(mut details) =
						self.get_manga_update(params, manga.clone(), false, true)
						&& let Some(chapters) = details.chapters.take()
						&& let Some(mut chapter) = chapters
							.iter()
							.max_by(|a, b| {
								a.date_uploaded.cmp(&b.date_uploaded).then_with(|| {
									a.chapter_number
										.partial_cmp(&b.chapter_number)
										.unwrap_or(core::cmp::Ordering::Equal)
								})
							})
							.cloned()
					{
						// This label is only used in the home preview, never in the saved chapter list.
						let count = chapters.len();
						chapter.title = Some(if chapter.chapter_number.is_some() {
							format!("{count} capitoli totali")
						} else {
							format!(
								"{} · {count} capitoli totali",
								chapter.title.as_deref().unwrap_or("Ultimo capitolo")
							)
						});
						entries.push(MangaWithChapter {
							manga: details,
							chapter,
						});
					}
				}
				if entries.is_empty() {
					HomeComponentValue::Scroller {
						entries: result
							.entries
							.into_iter()
							.map(Into::into)
							.collect::<Vec<Link>>(),
						listing: Some(listing),
					}
				} else {
					HomeComponentValue::MangaChapterList {
						page_size: Some(5),
						entries,
						listing: Some(listing),
					}
				}
			} else if id == "popular" || id == "rated" {
				HomeComponentValue::MangaList {
					ranking: true,
					page_size: Some(5),
					entries: result
						.entries
						.into_iter()
						.map(Into::into)
						.collect::<Vec<Link>>(),
					listing: Some(listing),
				}
			} else {
				HomeComponentValue::Scroller {
					entries: result
						.entries
						.into_iter()
						.map(Into::into)
						.collect::<Vec<Link>>(),
					listing: Some(listing),
				}
			};
			components.push(HomeComponent {
				title: Some(title.into()),
				subtitle: if id == "latest" {
					Some("Ultimi capitoli pubblicati".into())
				} else {
					None
				},
				value,
			});
		}
		Ok(HomeLayout { components })
	}

	fn parse_manga_page(&self, params: &Params, url: String) -> Result<MangaPageResult> {
		let html = Request::get(url)?.html()?;

		let entries = html
			.select(".archive-grid .archive-card, article.card, div.comics-grid .entry")
			.map(|els| {
				els.filter_map(|el| {
					let link = el
						.select_first(".card-title a")
						.or_else(|| el.select_first("a"))?;
					Some(Manga {
						key: link
							.attr("abs:href")?
							.strip_prefix_or_self(&params.base_url)
							.trim_end_matches("/")
							.into(),
						title: link
							.attr("title")
							.filter(|s| !s.is_empty())
							.or_else(|| link.text())?,
						cover: el
							.select_first(".card-cover img, a.thumb img")
							.and_then(|img| img.attr("abs:src")),
						..Default::default()
					})
				})
				.collect::<Vec<_>>()
			})
			.unwrap_or_default();
		let has_next_page = html.select_first(".pager a[rel=next]").is_some()
			|| (html.select_first("div.comics-grid").is_some() && entries.len() == 16);

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn get_manga_update(
		&self,
		params: &Params,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let manga_url = format!("{}{}", params.base_url, manga.key);
		let html = Request::get(&manga_url)?.html()?;

		if needs_details {
			let info_element = html
				.select_first(".manga-info, div.comic-info")
				.ok_or(error!("Page not found"))?;

			manga.title = info_element
				.select_first("h1.manga-title, h1.name")
				.and_then(|h1| h1.text())
				.unwrap_or(manga.title);
			manga.cover = html
				.select_first(".manga-cover img, div.comic-info .thumb > img")
				.and_then(|img| img.attr("abs:src"))
				.or(manga.cover);
			manga.artists = info_element
				.select_first("a[href*='artist=']")
				.and_then(|el| el.text())
				.map(|s| vec![s]);
			manga.authors = info_element
				.select_first("a[href*='author=']")
				.and_then(|el| el.text())
				.map(|s| vec![s]);
			let mut description = String::new();
			if let Some(year) = info_element
				.select_first("a[href*='year=']")
				.and_then(|el| el.text())
			{
				description.push_str(&format!("Anno di uscita: {year}\n\n"));
			}
			if let Some(plot) = html
				.select_first(".manga-plot, div#noidungm")
				.and_then(|div| div.text())
			{
				description.push_str(&plot);
			}
			if let Some(alternative) = info_element
				.select_first(".manga-alt")
				.and_then(|el| el.text())
			{
				description.push_str(&format!("\n\n{alternative}"));
			}
			manga.description = (!description.is_empty()).then_some(description);
			manga.url = Some(manga_url.clone());
			manga.tags = info_element
				.select(".hero-chips a, a[href*='type='], div.meta-data a.badge")
				.map(|els| els.filter_map(|el| el.text()).collect());
			manga.status = info_element
				.select_first("a[href*='status=']")
				.and_then(|el| el.text())
				.map(|text| match text.to_lowercase().as_str() {
					"in corso" => MangaStatus::Ongoing,
					"finito" => MangaStatus::Completed,
					"in pausa" => MangaStatus::Hiatus,
					"cancellato" => MangaStatus::Cancelled,
					_ => MangaStatus::Unknown,
				})
				.unwrap_or_default();
			let tags = manga.tags.as_deref().unwrap_or(&[]);
			manga.content_rating = if tags
				.iter()
				.any(|e| matches!(e.as_str(), "Adulti" | "Hentai" | "Maturo" | "Smut"))
			{
				ContentRating::NSFW
			} else if tags.iter().any(|e| e == "Ecchi") {
				ContentRating::Suggestive
			} else {
				ContentRating::Safe
			};
			manga.viewer = info_element
				.select_first("a[href*='type=']")
				.and_then(|el| el.text())
				.map(|text| match text.to_lowercase().as_str() {
					"manga" | "doujinshi" | "oneshot" => Viewer::RightToLeft,
					"manhua" | "manhwa" => Viewer::Webtoon,
					_ => Viewer::Unknown,
				})
				.unwrap_or_default();

			send_partial_result(&manga);
		}

		if needs_chapters {
			fn get_chapter_number(id: &str) -> Option<f32> {
				id.chars()
					.filter(|a| (*a >= '0' && *a <= '9') || *a == ' ' || *a == '.')
					.collect::<String>()
					.split(' ')
					.collect::<Vec<&str>>()
					.into_iter()
					.map(|a| a.parse::<f32>().unwrap_or(0.0))
					.find(|a| *a > 0.0)
			}

			manga.chapters = html
				.select(".volume-list .ch-row, .chapters-wrapper .chapter")
				.map(|els| {
					els.filter_map(|el| {
						let url = el.select_first("a.ch, a.chap")?.attr("abs:href")?;
						let title = el.select_first(".ch-name, span.d-inline-block")?.text()?;
						let chapter_number = get_chapter_number(&title);
						Some(Chapter {
							key: url.strip_prefix_or_self(&params.base_url).into(),
							title: if chapter_number.is_some()
								&& title.to_lowercase().starts_with("capitolo ")
								&& title.split_whitespace().count() == 2
							{
								None
							} else {
								Some(title)
							},
							chapter_number,
							date_uploaded: el
								.select(".ch-date, .chap-date, time, [data-timestamp], [data-date]")
								.and_then(|els| {
									els.filter_map(|date| {
										date.attr("datetime")
											.and_then(|s| parse_chapter_date(&s))
											.or_else(|| {
												date.attr("data-timestamp")
													.and_then(|s| parse_chapter_date(&s))
											})
											.or_else(|| {
												date.attr("data-date")
													.and_then(|s| parse_chapter_date(&s))
											})
											.or_else(|| {
												date.attr("title")
													.and_then(|s| parse_chapter_date(&s))
											})
											.or_else(|| {
												date.text().and_then(|s| parse_chapter_date(&s))
											})
									})
									.next_back()
								}),
							url: Some(url),
							..Default::default()
						})
					})
					.collect()
				});
		}

		Ok(manga)
	}

	fn get_page_list(&self, params: &Params, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let key = chapter.key.replace("style=pages", "style=list");
		let separator = if key.contains('?') { "&" } else { "?" };
		let url = if key.contains("style=list") {
			format!("{}{}", params.base_url, key)
		} else {
			format!("{}{}{separator}style=list", params.base_url, key)
		};
		let html = Request::get(url)?.html()?;

		Ok(html
			.select("img.reader-page, div#page img.page-image")
			.map(|els| {
				els.filter_map(|el| {
					Some(Page {
						content: PageContent::url(el.attr("abs:src")?),
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default())
	}

	fn get_image_request(
		&self,
		params: &Params,
		url: String,
		_context: Option<PageContext>,
	) -> Result<Request> {
		// this doesn't look like it's necessary, but it's here for the possible future
		Ok(Request::get(url)?.header("Referer", &format!("{}/", params.base_url)))
	}

	fn handle_deep_link(&self, params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(params.base_url.as_ref()) else {
			return Ok(None);
		};

		const MANGA_PATH: &str = "/manga/";
		if !path.starts_with(MANGA_PATH) {
			return Ok(None);
		}

		let slash_count = path.matches('/').count();

		if slash_count <= 3 {
			// ex: https://www.mangaworld.me/manga/2980/babylon-made-wa-nan-kounen
			Ok(Some(DeepLinkResult::Manga { key: path.into() }))
		} else {
			// ex: https://www.mangaworld.me/manga/2980/babylon-made-wa-nan-kounen/read/64a7eab08c9c4e3f780c5932/1
			Ok(None)
		}
	}
}

fn parse_chapter_date(text: &str) -> Option<i64> {
	let text = text.trim();
	// The desktop test runner does not support localized month names or date-only
	// formats. Normalize Italian dates, keeping a real time if one is provided.
	let normalized = text.to_lowercase();
	let mut normalized = normalized.replace(',', "").replace(" alle ", " ");
	for (month, number) in [
		("gennaio", "01"),
		("febbraio", "02"),
		("marzo", "03"),
		("aprile", "04"),
		("maggio", "05"),
		("giugno", "06"),
		("luglio", "07"),
		("agosto", "08"),
		("settembre", "09"),
		("ottobre", "10"),
		("novembre", "11"),
		("dicembre", "12"),
	] {
		normalized = normalized.replace(month, number);
	}
	let parts: Vec<&str> = normalized.split_whitespace().collect();
	if parts.len() >= 3
		&& parts[0].parse::<u32>().is_ok()
		&& parts[1].parse::<u32>().is_ok()
		&& parts[2].len() == 4
	{
		let time = parts.get(3).copied().unwrap_or("00:00");
		let candidate = format!("{}-{}-{} {time}", parts[2], parts[1], parts[0]);
		for format in ["yyyy-MM-dd HH:mm:ss", "yyyy-MM-dd HH:mm"] {
			if let Some(date) = parse_date_with_options(&candidate, format, "en_US", "Europe/Rome")
			{
				return Some(date);
			}
		}
	}
	if normalized.len() == 10 {
		for format in ["dd/MM/yyyy HH:mm", "yyyy-MM-dd HH:mm", "dd-MM-yyyy HH:mm"] {
			if let Some(date) = parse_date_with_options(
				format!("{normalized} 00:00"),
				format,
				"en_US",
				"Europe/Rome",
			) {
				return Some(date);
			}
		}
	}
	if let Ok(timestamp) = text.parse::<i64>() {
		if (1_000_000_000..10_000_000_000).contains(&timestamp) {
			return Some(timestamp);
		}
		if (1_000_000_000_000..10_000_000_000_000).contains(&timestamp) {
			return Some(timestamp / 1000);
		}
	}
	for format in [
		"yyyy-MM-dd'T'HH:mm:ss.SSSXXX",
		"yyyy-MM-dd'T'HH:mm:ssXXX",
		"yyyy-MM-dd HH:mm:ss",
		"d MMMM yyyy HH:mm",
		"d MMMM yyyy, HH:mm",
		"d MMMM yyyy",
		"dd/MM/yyyy HH:mm",
		"dd/MM/yyyy",
		"yyyy-MM-dd",
	] {
		if let Some(date) = parse_date_with_options(text, format, "it", "Europe/Rome") {
			return Some(date);
		}
	}
	let lower = text.to_lowercase();
	let now = current_date();
	if lower == "adesso" || lower == "ora" || lower == "just now" {
		return Some(now);
	}
	if lower == "ieri" {
		return Some(now - 86400);
	}
	let mut parts = lower.split_whitespace();
	let number = match parts.next()? {
		"un" | "una" => 1,
		value => value.parse::<i64>().ok()?,
	};
	if number < 0 || parts.next_back()? != "fa" {
		return None;
	}
	let seconds = match parts.next()? {
		"secondo" | "secondi" => 1,
		"minuto" | "minuti" => 60,
		"ora" | "ore" => 3600,
		"giorno" | "giorni" => 86400,
		"settimana" | "settimane" => 604800,
		_ => return None,
	};
	now.checked_sub(number.checked_mul(seconds)?)
}
