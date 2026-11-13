use crate::{
	Impl, Params, helpers,
	models::{Post, SearchResponse},
	settings,
};
use aidoku::{
	Chapter, HomeComponent, HomeComponentValue, HomeLayout, Link, LinkValue, Manga,
	MangaWithChapter, Result,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::{
		html::{Document, Element},
		net::Request,
		std::current_date,
	},
	prelude::*,
};

pub(crate) fn get_home<T: Impl + ?Sized>(source: &T, params: &Params) -> Result<HomeLayout> {
	let label_novels = settings::label_novels();
	let mut urls = vec![params.base_url.to_string()];
	for (sort, series_type) in [("createdAt", None), (params.sorts[0], Some("NOVEL"))] {
		let mut query = QueryParameters::new();
		query.push("page", Some("1"));
		query.push("perPage", Some("18"));
		query.push("view", Some("archive"));
		query.push("orderBy", Some(sort));
		query.push("orderDirection", Some("desc"));
		if let Some(series_type) = series_type {
			query.push("seriesType", Some(series_type));
		}
		urls.push(format!("{}/api/query?{query}", params.api_url));
	}
	// Pinned titles can fall below the first API page.
	urls.push(format!("{}/api/posts?page=1&perPage=100", params.api_url));
	let requests = urls
		.iter()
		.map(|url| {
			let request = helpers::request(params, url)?;
			Ok(if url == &params.base_url {
				request.header("Accept", "text/html")
			} else {
				request
			})
		})
		.collect::<Result<Vec<_>>>()?;
	let mut layout = HomeLayout::default();
	let mut failure = None;
	for (index, response) in Request::send_all(requests).into_iter().enumerate() {
		let result = (|| -> Result<()> {
			let response = helpers::check_response(response?, &urls[index])?;
			if index == 0 {
				layout = source.parse_home(params, &response.get_html()?)?;
			} else {
				let data: SearchResponse = response.get_json_owned()?;
				if index + 1 == urls.len() {
					update_home_metadata(&mut layout, data.posts, params)?;
					return Ok(());
				}
				let entries: Vec<Link> = data
					.posts
					.iter()
					.filter_map(|post| post.to_manga(label_novels).ok())
					.map(Link::from)
					.collect();
				if !entries.is_empty() {
					let component = HomeComponent {
						title: Some(
							if index == 1 {
								params.new_releases_title
							} else {
								params.novels_title
							}
							.into(),
						),
						value: HomeComponentValue::Scroller {
							entries,
							listing: None,
						},
						..Default::default()
					};
					if let Some(existing) = layout
						.components
						.iter_mut()
						.find(|c| c.title == component.title)
					{
						*existing = component;
						return Ok(());
					}
					let index = layout
						.components
						.iter()
						.position(|c| {
							matches!(c.title.as_deref(), Some("Most Popular" | "Tendencias"))
						})
						.unwrap_or(layout.components.len());
					layout.components.insert(index, component);
				}
			}
			Ok(())
		})();
		if let Err(error) = result {
			println!("Home request {} failed: {error:?}", urls[index]);
			failure = Some(error);
		}
	}
	if layout.components.is_empty() {
		return Err(failure.unwrap_or_else(|| error!("Home sections not found")));
	}
	Ok(layout)
}

pub(crate) fn update_home_metadata(
	layout: &mut HomeLayout,
	mut posts: Vec<Post>,
	params: &Params,
) -> Result<()> {
	let label_novels = settings::label_novels();
	for component in &mut layout.components {
		if let HomeComponentValue::BigScroller { entries, .. } = &mut component.value {
			for entry in entries {
				let slug = entry.key.strip_prefix("/series/").unwrap_or(&entry.key);
				if let Some(index) = posts.iter().position(|post| post.slug == slug) {
					let mut updated = posts[index].to_featured(&params.base_url, label_novels)?;
					if updated.description.is_none() {
						updated.description = entry.description.take();
					}
					if updated.tags.is_none() {
						updated.tags = entry.tags.take();
						if let Some(tags) = &updated.tags {
							updated.content_rating = helpers::content_rating(
								tags,
								updated.content_rating == aidoku::ContentRating::NSFW,
							);
						}
					}
					if updated.viewer == aidoku::Viewer::Unknown {
						updated.viewer = entry.viewer;
					}
					*entry = updated;
				}
			}
			continue;
		}

		if !matches!(
			component.title.as_deref(),
			Some("Latest Releases" | "Latest Updates" | "Últimas Actualizaciones")
		) {
			continue;
		}

		if let HomeComponentValue::MangaChapterList { entries, .. } = &mut component.value {
			for entry in entries {
				let slug = entry
					.manga
					.key
					.strip_prefix("/series/")
					.unwrap_or(&entry.manga.key);
				let Some(index) = posts.iter().position(|post| post.slug == slug) else {
					continue;
				};
				if let Some(updated) = posts
					.remove(index)
					.into_manga_with_chapter(&params.base_url, label_novels)?
					&& updated.chapter.url == entry.chapter.url
				{
					*entry = updated;
				}
			}
			continue;
		}
		let HomeComponentValue::MangaList { entries, .. } = &component.value else {
			continue;
		};
		let mut chapters = Vec::new();
		for link in entries {
			let Some(LinkValue::Manga(manga)) = &link.value else {
				continue;
			};
			let slug = manga.key.strip_prefix("/series/").unwrap_or(&manga.key);
			let index = posts
				.iter()
				.position(|post| post.slug == slug)
				.ok_or_else(|| error!("Latest series missing from API: {slug}"))?;
			if let Some(entry) = posts
				.remove(index)
				.into_manga_with_chapter(&params.base_url, label_novels)?
			{
				chapters.push(entry);
			}
		}
		if chapters.is_empty() {
			bail!("No accessible latest chapters");
		}
		component.value = HomeComponentValue::MangaChapterList {
			page_size: Some(5),
			entries: chapters,
			listing: None,
		};
	}
	Ok(())
}

fn latest_chapter(link: &Element, params: &Params) -> Option<Chapter> {
	let card = link.parent()?;
	let row = card.has_class("newly-free-chapter-row");
	if row && card.has_class("paid-chapter-row") {
		return None;
	}
	let chapter = if row {
		card.select("a[href*='/series/']")?.find(|link| {
			link.attr("href").is_some_and(|href| {
				href.strip_prefix(params.base_url.as_ref())
					.unwrap_or(&href)
					.matches('/')
					.count() == 3
			})
		})?
	} else {
		card.select("a.newly-free-chapter-row")?.find(|link| {
			!link.has_class("paid-chapter-row") && link.select_first(".lucide-lock").is_none()
		})?
	};
	let href = chapter.attr("href")?;
	let key = href.strip_prefix(params.base_url.as_ref()).unwrap_or(&href);
	let label = chapter
		.attr("title")
		.or_else(|| chapter.select_first("span").and_then(|span| span.text()))
		.or_else(|| chapter.text())?;
	let label = label.trim().strip_prefix("Chapter ")?;
	let end = label
		.find(|c: char| !c.is_ascii_digit() && c != '.')
		.unwrap_or(label.len());
	let number = label[..end].parse().ok()?;
	let title = label[end..].trim_start_matches([' ', ':', '-']).trim();
	let scope = if row { &card } else { &chapter };
	let date_uploaded = scope
		.select_first("time")
		.and_then(|time| time.attr("datetime"))
		.and_then(|date| chrono::DateTime::parse_from_rfc3339(&date).ok())
		.map(|date| date.timestamp())
		.or_else(|| {
			let text = scope
				.select("time, span")?
				.filter_map(|span| span.text())
				.find(|text| text.trim().ends_with("ago"))?;
			let (amount, unit) = text.trim().strip_suffix("ago")?.trim().split_once(' ')?;
			let amount: i64 = amount.parse().ok()?;
			let seconds = if unit.starts_with("second") {
				1
			} else if unit.starts_with("min") {
				60
			} else if unit.starts_with("hour") {
				3600
			} else if unit.starts_with("day") {
				86400
			} else if unit.starts_with("week") {
				604800
			} else if unit.starts_with("month") {
				2592000
			} else if unit.starts_with("year") {
				31536000
			} else {
				return None;
			};
			Some(current_date() - amount * seconds)
		});
	Some(Chapter {
		key: key.into(),
		url: Some(format!("{}{key}", params.base_url)),
		title: (!title.is_empty()).then(|| title.into()),
		chapter_number: Some(number),
		date_uploaded,
		..Default::default()
	})
}

fn manga(element: &Element, params: &Params, label_novels: bool) -> Option<Manga> {
	let child;
	let link = if element.attr("href").is_some() {
		element
	} else {
		child = element.select_first("a[href*='/series/']")?;
		&child
	};
	let href = link.attr("href")?;
	let path = href
		.strip_prefix(params.base_url.as_ref())
		.unwrap_or(&href)
		.strip_prefix("/series/")?;
	let slug = path
		.split(['/', '?', '#'])
		.next()
		.filter(|s| !s.trim().is_empty())?;
	let title = link
		.attr("title")
		.filter(|s| !s.trim().is_empty())
		.or_else(|| {
			element
				.select_first("h2, h3, p.display-6")
				.and_then(|e| e.text())
		})
		.or_else(|| element.select_first("img").and_then(|e| e.attr("alt")))?;
	if title.trim().is_empty() {
		return None;
	}
	Some(Manga {
		key: format!("/series/{slug}"),
		title: helpers::display_title(
			&title,
			element
				.select_first(".uppercase")
				.and_then(|kind| kind.text())
				.is_some_and(|kind| kind.eq_ignore_ascii_case("novel")),
			label_novels,
		),
		cover: element.select_first("img").and_then(|img| {
			img.attr("data-fallback-src")
				.filter(|s| s.starts_with("https://") && !s.contains("no-cover"))
				.or_else(|| img.attr("abs:src"))
		}),
		..Default::default()
	})
}

pub fn parse_home(html: &Document, params: &Params) -> Result<HomeLayout> {
	let label_novels = settings::label_novels();
	let mut components = Vec::new();
	let mut slides: Vec<_> = html
		.select(&params.home_slider_selector)
		.map(|slides| {
			slides
				.filter_map(|e| {
					let mut manga = manga(&e, params, label_novels)?;
					if params.banner_width.zip(params.banner_height).is_some() {
						manga.cover = e
							.select_first("img")
							.and_then(|image| image.attr("abs:src"));
					}
					manga.description = e
						.select_first("h3 + p, .text-lg, .break-words, .em-hero-copy p.d-none")
						.and_then(|p| p.text())
						.map(|s| s.trim().into())
						.filter(|s: &String| !s.is_empty());
					let mut tags: Vec<String> = e
						.select("span.capitalize, span.font-light")
						.into_iter()
						.flatten()
						.filter_map(|tag| tag.text())
						.map(|tag| tag.trim().into())
						.filter(|tag: &String| !tag.is_empty())
						.collect();
					tags.sort_unstable();
					tags.dedup();
					let kind = e
						.select_first(".uppercase")
						.and_then(|kind| kind.text())
						.map(|kind| kind.trim().to_uppercase());

					manga.content_rating = helpers::content_rating(&tags, false);
					manga.viewer = helpers::viewer(kind.as_deref());
					manga.tags = (!tags.is_empty()).then_some(tags);
					let index = e
						.attr("data-swiper-slide-index")
						.and_then(|s| s.parse::<usize>().ok())
						.unwrap_or(0);
					Some((index, manga))
				})
				.collect()
		})
		.unwrap_or_default();
	// Looping carousels can rotate and duplicate their DOM slides.
	slides.sort_by_key(|(index, _)| *index);
	let mut entries: Vec<Manga> = Vec::new();
	for (_, manga) in slides {
		if !entries.iter().any(|entry| entry.key == manga.key) {
			entries.push(manga);
		}
	}
	if !entries.is_empty() {
		components.push(HomeComponent {
			value: if let Some((width, height)) = params.banner_width.zip(params.banner_height) {
				HomeComponentValue::ImageScroller {
					links: entries.into_iter().map(Link::from).collect(),
					width: Some(width),
					height: Some(height),
					auto_scroll_interval: None,
				}
			} else {
				HomeComponentValue::BigScroller {
					entries,
					auto_scroll_interval: None,
				}
			},
			..Default::default()
		});
	}
	if let Some(headings) = html.select("main h1, main h2, main h3") {
		for heading in headings {
			let Some(title) = heading
				.text()
				.map(|s| s.trim().into())
				.filter(|s: &String| params.home_section_titles.contains(&s.as_str()))
			else {
				continue;
			};
			if components
				.iter()
				.any(|c| c.title.as_deref() == Some(&title))
			{
				continue;
			}
			let mut parent = heading.parent();
			let mut entries: Vec<Link> = Vec::new();
			let mut chapters: Vec<MangaWithChapter> = Vec::new();
			for _ in 0..3 {
				let Some(block) = parent else {
					break;
				};
				if block.tag_name().as_deref() == Some("main") {
					break;
				}
				let neighbors = block.select("h1, h2, h3").is_some_and(|hs| {
					hs.filter_map(|h| h.text()).any(|t| {
						t.trim() != title
							&& (params.home_section_titles.contains(&t.trim())
								|| t.trim() == "Latest Updates")
					})
				});
				if neighbors {
					break;
				}
				if let Some(cards) = block.select("a[href*='/series/']") {
					for card in cards {
						if card.select_first("img").is_none() {
							continue;
						}
						if let Some(manga) = manga(&card, params, label_novels)
							&& !entries.iter().any(|entry| {
								matches!(&entry.value, Some(LinkValue::Manga(existing)) if existing.key == manga.key)
							})
						{
							if matches!(title.as_str(), "Latest Releases" | "Latest Updates")
        && let Some(chapter) = latest_chapter(&card, params)
       {
        if !chapters.iter().any(|entry| entry.chapter.key == chapter.key) {
         chapters.push(MangaWithChapter { manga, chapter });
        }
       } else {
        entries.push(Link::from(manga));
       }
						}
					}
				}
				if !entries.is_empty() || !chapters.is_empty() {
					break;
				}
				parent = block.parent();
			}
			if entries.is_empty() && chapters.is_empty() {
				continue;
			}
			let ranking = matches!(title.as_str(), "Most Popular" | "Tendencias");
			let paged = ranking
				|| matches!(
					title.as_str(),
					"Latest Releases" | "Latest Updates" | "Últimas Actualizaciones"
				);
			components.push(HomeComponent {
				title: Some(title),
				value: if !chapters.is_empty() {
					HomeComponentValue::MangaChapterList {
						page_size: Some(5),
						entries: chapters,
						listing: None,
					}
				} else if paged {
					HomeComponentValue::MangaList {
						ranking,
						page_size: Some(if ranking { 3 } else { 5 }),
						entries,
						listing: None,
					}
				} else {
					HomeComponentValue::Scroller {
						entries,
						listing: None,
					}
				},
				..Default::default()
			});
		}
	}
	if components.is_empty() {
		bail!("Home sections not found");
	}
	Ok(HomeLayout { components })
}
