use aidoku::{
	Chapter, ContentRating, Manga, MangaPageResult, MangaStatus, Viewer,
	alloc::{String, Vec},
	imports::{
		html::{Document, Element},
		std::parse_date,
	},
	prelude::*,
};

use crate::helpers::{chapter_key, chapter_url, manga_key, manga_url};

/// The pagination ends in a `→` link, which is a plain `<span>` on the last page.
pub fn has_next_page(html: &Document) -> bool {
	html.select(".page-btn--nav")
		.and_then(|mut buttons| buttons.next_back())
		.and_then(|button| button.tag_name())
		.is_some_and(|tag| tag == "a")
}

/// Series cards of `/library`.
pub fn parse_library(html: &Document) -> MangaPageResult {
	MangaPageResult {
		entries: parse_manga_cards(html, ".lib-grid a.comic-card"),
		has_next_page: has_next_page(html),
	}
}

pub fn parse_manga_cards(html: &Document, selector: &str) -> Vec<Manga> {
	html.select(selector)
		.map(|cards| cards.filter_map(|card| parse_manga_card(&card)).collect())
		.unwrap_or_default()
}

fn parse_manga_card(card: &Element) -> Option<Manga> {
	// The site also has novels, which can't be read here.
	let kind = card
		.select_first("span.absolute.top-2.left-2")
		.and_then(|badge| badge.text());
	if kind.is_some_and(|kind| kind.contains("Novel")) {
		return None;
	}
	let key = manga_key(&card.attr("abs:href")?)?;
	Some(Manga {
		title: card.select_first("p.leading-snug")?.text()?,
		cover: card.select_first("img").and_then(|img| img.attr("abs:src")),
		url: Some(manga_url(&key)),
		key,
		..Default::default()
	})
}

/// The featured series at the top of the home page.
pub fn parse_hero(html: &Document) -> Vec<Manga> {
	html.select("#heroSection .hero-slide")
		.map(|slides| {
			slides
				.filter_map(|slide| {
					let kind = slide
						.select_first(".hero-badge")
						.and_then(|badge| badge.text());
					if kind.is_some_and(|kind| kind.contains("Novel")) {
						return None;
					}
					let key =
						manga_key(&slide.select_first("a.hero-btn[href]")?.attr("abs:href")?)?;
					// The slide only has a wide banner, so use the series' cover from
					// elsewhere on the page when it's there.
					let cover = html
						.select(format!("img[src^='/uploads/covers/{key}/']"))
						.and_then(|mut images| {
							images.find_map(|img| {
								img.attr("abs:src").filter(|src| !src.contains("/banner-"))
							})
						})
						// Only the first banner is in `src`; the rest are lazy loaded.
						.or_else(|| {
							slide
								.select_first("img[data-src]")
								.and_then(|img| img.attr("abs:data-src"))
						});
					Some(Manga {
						title: slide.select_first(".hero-title")?.text()?,
						cover,
						description: slide
							.select_first(".hero-desc")
							.and_then(|desc| desc.text()),
						url: Some(manga_url(&key)),
						key,
						..Default::default()
					})
				})
				.collect()
		})
		.unwrap_or_default()
}

/// The series cards of the home page section with this heading.
pub fn parse_home_section(html: &Document, heading: &str) -> Vec<Manga> {
	html.select("section")
		.and_then(|mut sections| {
			sections.find(|section| {
				section
					.select_first("h2")
					.and_then(|h2| h2.text())
					.is_some_and(|title| title == heading)
			})
		})
		.and_then(|section| section.select("a.comic-card"))
		.map(|cards| cards.filter_map(|card| parse_manga_card(&card)).collect())
		.unwrap_or_default()
}

/// The "Más populares" widget of the home page, most viewed first.
pub fn parse_ranking(html: &Document) -> Vec<Manga> {
	// The widget is on the page twice, for wide and narrow screens.
	html.select_first(".mp-widget")
		.and_then(|widget| widget.select(".mp-panel[data-panel=today] a[href]"))
		.map(|links| {
			links
				.filter_map(|link| {
					let key = manga_key(&link.attr("abs:href")?)?;
					Some(Manga {
						title: link.select_first(".mp-title")?.text()?,
						cover: link.select_first("img").and_then(|img| img.attr("abs:src")),
						url: Some(manga_url(&key)),
						key,
						..Default::default()
					})
				})
				.collect()
		})
		.unwrap_or_default()
}

/// The "Actualizados recientemente" section of the home page: each series
/// with its newest chapter that can be read for free.
pub fn parse_recent_chapters(html: &Document) -> Vec<(Manga, Chapter)> {
	html.select("#ru-list article")
		.map(|rows| {
			rows.filter_map(|row| {
				let link = row.select_first("a[title][href]")?;
				let key = manga_key(&link.attr("abs:href")?)?;
				let manga = Manga {
					title: link.attr("title")?,
					cover: row.select_first("img").and_then(|img| img.attr("abs:src")),
					url: Some(manga_url(&key)),
					key,
					..Default::default()
				};

				// Locked chapters are buttons, so only links can be read.
				let link = row.select_first("li a[href]")?;
				let key = chapter_key(&link.attr("abs:href")?)?;
				let label = link.select_first("span")?.text()?;
				let (chapter_number, title) = parse_chapter_label(&label);
				let chapter = Chapter {
					title,
					chapter_number,
					date_uploaded: link
						.select_first("[data-ts]")
						.and_then(|date| parse_site_date(&date.attr("data-ts")?)),
					url: Some(chapter_url(&key)),
					key,
					..Default::default()
				};
				Some((manga, chapter))
			})
			.collect()
		})
		.unwrap_or_default()
}

/// Dates look like `2026-10-22T20:11:56.921Z`, always in UTC. The milliseconds
/// are dropped.
fn parse_site_date(date: &str) -> Option<i64> {
	let date = date.get(..19)?.replacen('T', " ", 1);
	parse_date(date, "yyyy-MM-dd HH:mm:ss")
}

/// Splits `Capítulo 12.5: Epílogo` (or `Cap. 12.5`) into the number and the
/// rest of the title.
fn parse_chapter_label(label: &str) -> (Option<f32>, Option<String>) {
	let label = label.trim();
	let Some(rest) = label
		.strip_prefix("Capítulo")
		.or_else(|| label.strip_prefix("Cap."))
	else {
		return (None, Some(label.into()).filter(|t: &String| !t.is_empty()));
	};
	let rest = rest.trim_start();
	let (number, title) = rest.split_once(':').unwrap_or((rest, ""));
	let title = title.trim();
	(
		number.trim().parse().ok(),
		Some(title.into()).filter(|t: &String| !t.is_empty()),
	)
}

/// Fills in a series from its page.
pub fn parse_manga_details(html: &Document, manga: &mut Manga) {
	if let Some(title) = html.select_first("h1").and_then(|h1| h1.text()) {
		manga.title = title;
	}
	// The first image is the banner and the series cards below it have their
	// own covers, so the cover is the one named after the series.
	if let Some(cover) = html
		.select(".comic-page-wrap img.object-cover[alt]")
		.and_then(|mut images| {
			images.find(|img| img.attr("alt").is_some_and(|alt| alt == manga.title))
		})
		.and_then(|img| img.attr("abs:src"))
	{
		manga.cover = Some(cover);
	}
	manga.description = html
		.select_first("#comic-description")
		.and_then(|description| description.text())
		.filter(|text| !text.is_empty());

	// Rows like "Estado: En emisión", "Demográfico: Shounen" or "Autor: …".
	let mut status = None;
	let mut authors = Vec::new();
	let mut artists = Vec::new();
	if let Some(rows) = html.select(".flex.items-baseline.justify-between.gap-2") {
		for row in rows {
			let mut spans = row.select("span").into_iter().flatten();
			let (Some(label), Some(value)) = (
				spans.next().and_then(|span| span.text()),
				spans.next().and_then(|span| span.text()),
			) else {
				continue;
			};
			match label.as_str() {
				"Estado" => status = Some(value),
				"Autor" => authors.push(value),
				"Arte" => artists.push(value),
				_ => {}
			}
		}
	}

	// The badges under the title are the status, the demographic and the type.
	let badges: Vec<String> = html
		.select(".comic-page-wrap span.inline-flex.items-center.rounded")
		.map(|badges| badges.filter_map(|badge| badge.text()).collect())
		.unwrap_or_default();

	manga.status = match status
		.as_deref()
		.or_else(|| badges.first().map(String::as_str))
		.map(str::to_lowercase)
		.as_deref()
	{
		Some("en emisión") => MangaStatus::Ongoing,
		Some("completado") => MangaStatus::Completed,
		Some("en pausa") => MangaStatus::Hiatus,
		Some("cancelado") => MangaStatus::Cancelled,
		_ => MangaStatus::Unknown,
	};

	let tags: Vec<String> = badges
		.into_iter()
		.filter(|badge| {
			!matches!(
				badge.to_lowercase().as_str(),
				"en emisión" | "completado" | "en pausa" | "cancelado"
			)
		})
		.collect();

	// Without an author, credit the group that translates it, as the site does.
	// Several groups are listed in a dropdown, a single one is a plain link.
	// Other group links further down belong to other series.
	if authors.is_empty() {
		authors = match html.select("#groups-dropdown-panel a[href^='/groups/']") {
			Some(links) if !links.is_empty() => {
				links.filter_map(|link| group_name(&link)).collect()
			}
			_ => html
				.select_first(".comic-page-wrap a[href^='/groups/']")
				.and_then(|link| group_name(&link))
				.into_iter()
				.collect(),
		};
	}
	manga.authors = Some(authors).filter(|authors| !authors.is_empty());
	manga.artists = Some(artists).filter(|artists| !artists.is_empty());

	// The site puts series rated "Adulto" behind an age check.
	manga.content_rating = if html
		.select_first(".adult-gate-notice, #age-verification-modal")
		.is_some()
	{
		ContentRating::NSFW
	} else if tags
		.iter()
		.any(|tag| matches!(tag.as_str(), "+15" | "Ecchi"))
	{
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	};

	manga.viewer = if tags.iter().any(|tag| tag == "Manhwa" || tag == "Manhua") {
		Viewer::Webtoon
	} else if tags.iter().any(|tag| tag == "Manga") {
		Viewer::RightToLeft
	} else {
		Viewer::Unknown
	};

	manga.tags = Some(tags).filter(|tags| !tags.is_empty());
	manga.url = Some(manga_url(&manga.key));
}

/// Groups without a logo have their initial as the avatar, inside the link.
fn group_name(link: &Element) -> Option<String> {
	let text = link.text()?;
	let name = match link.select_first("div").and_then(|avatar| avatar.text()) {
		Some(initial) => text.strip_prefix(initial.as_str()).unwrap_or(&text).trim(),
		None => text.trim(),
	};
	Some(name.into()).filter(|name: &String| !name.is_empty())
}

/// A page of `/comics/{slug}/chapters`, newest first.
///
/// Locked chapters (unlocked with coins or by watching an ad) are buttons
/// instead of links and can't be read, so they're skipped.
pub fn parse_chapters(html: &Document) -> Vec<Chapter> {
	html.select("a[data-chapter-id][href]")
		.map(|links| {
			links
				.filter_map(|link| {
					let key = chapter_key(&link.attr("abs:href")?)?;
					let (label_number, title) =
						parse_chapter_label(&link.attr("data-chapter-label").unwrap_or_default());
					Some(Chapter {
						title,
						chapter_number: link
							.attr("data-chapter-num")
							.and_then(|num| num.parse().ok())
							.or(label_number),
						date_uploaded: link
							.select_first("[data-ts]")
							.and_then(|date| parse_site_date(&date.attr("data-ts")?)),
						url: Some(chapter_url(&key)),
						key,
						..Default::default()
					})
				})
				.collect()
		})
		.unwrap_or_default()
}

/// The reader lazy loads images: `data-src` holds the url until it's moved to `src`.
pub fn parse_page_urls(html: &Document) -> Vec<String> {
	html.select("img.page-img")
		.map(|images| {
			images
				.filter_map(|img| {
					img.attr("abs:data-src")
						.filter(|src| !src.is_empty())
						.or_else(|| img.attr("abs:src"))
				})
				.collect()
		})
		.unwrap_or_default()
}

/// The reader links back to its series.
pub fn parse_reader_manga_key(html: &Document) -> Option<String> {
	html.select("a[href^='/comics/']")?
		.find_map(|link| manga_key(&link.attr("abs:href")?))
}
