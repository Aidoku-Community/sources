use crate::{
	BASE_URL, Webnex,
	helpers::{
		browse_page, browse_request, browse_table, card, current_year, link_filters, parse_key,
		parse_relative_date, parse_response, text,
	},
	rsc::{chunks, expand},
	settings::{self, CONTENT_RATINGS, CONTENT_TYPES, Catalogue},
};
use aidoku::{
	Chapter, Home, HomeComponent, HomeComponentValue, HomeLayout, Link, Listing, MangaWithChapter,
	Result,
	alloc::{String, Vec, string::ToString, vec},
	imports::{
		html::{Document, Html},
		net::Request,
		std::current_date,
	},
	prelude::*,
};

pub fn layout(doc: &Document, now: i64, shown: Option<&[String]>) -> HomeLayout {
	let mut chunks = chunks(doc);
	let mut components = Vec::new();
	for (id, selector) in [
		("popular", "section[aria-labelledby=popular-heading]"),
		("yours", "section[aria-labelledby$=-heading]"),
		("taste", "section[aria-labelledby$=-heading]"),
		("new", "section[aria-labelledby=new-heading]"),
		("trending", "section[aria-labelledby=trending-heading]"),
		("latest", "section[aria-labelledby=latest-heading]"),
		("manhwa", "section[aria-labelledby=manhwa-heading]"),
		("manga", "section[aria-labelledby=manga-heading]"),
		("manhua", "section[aria-labelledby=manhua-heading]"),
		("finished", "[role=tabpanel][id$=-finished]"),
		("added", "[role=tabpanel][id$=-added]"),
	] {
		if shown.is_some_and(|shown| !shown.iter().any(|value| value == id)) {
			continue;
		}
		// Personalised pages give "Your taste" the taste-heading id, so these two go by their titles.
		let Some(section) = (match id {
			"taste" | "yours" => {
				let prefix = if id == "taste" { "Readers" } else { "Your" };
				doc.select(selector).and_then(|sections| {
					sections.into_iter().find(|s| {
						s.select_first("h2")
							.and_then(|h| h.text())
							.is_some_and(|t| t.starts_with(prefix))
					})
				})
			}
			_ => doc.select_first(selector),
		}) else {
			continue;
		};
		let Some(row) = expand(&section, &mut chunks)
			.and_then(|html| Html::parse_with_url(html, BASE_URL).ok())
		else {
			continue;
		};
		let elements: Vec<_> = row
			.select("li.card-item, .side-list > li")
			.into_iter()
			.flatten()
			.collect();
		let title = match id {
			"popular" => Some("Popular This Week".into()),
			"taste" => Some("Readers Favourites".into()),
			"yours" => Some("Your Taste".into()),
			"latest" => Some("Latest Updates".into()),
			"trending" => Some("Recently Popular".into()),
			"finished" => Some("Finished".into()),
			"added" => Some("Just Added".into()),
			_ => row.select_first("h2").and_then(|el| el.text()),
		};
		let listing = row
			.select_first("a.see-all[href^='/browse']")
			.and_then(|a| a.attr("href"))
			.map(|id| Listing {
				id,
				name: title.clone().unwrap_or_default(),
				..Default::default()
			});
		let value = match id {
			"popular" => {
				let entries: Vec<_> = elements
					.iter()
					.filter_map(|el| {
						let mut manga = card(el)?;
						manga.tags = text(el, ".card-kind, .card-meta span:first-child")
							.filter(|s| matches!(s.as_str(), "Manga" | "Manhwa" | "Manhua"))
							.map(|s| vec![s]);
						Some(manga)
					})
					.collect();
				if entries.is_empty() {
					continue;
				}
				HomeComponentValue::BigScroller {
					entries,
					auto_scroll_interval: Some(5.0),
				}
			}
			"latest" => {
				let entries: Vec<_> = elements
					.iter()
					.filter_map(|el| {
						let link = el.select_first(".card-chapters a[href^='/read/']")?;
						let key = parse_key(&link.attr("href")?, "/read/")?.to_string();
						let label = text(&link, ".card-ch-label")?;
						let n = label.trim_start_matches("Ch.").parse().ok();
						Some(MangaWithChapter {
							manga: card(el)?,
							chapter: Chapter {
								key,
								chapter_number: n,
								title: n.is_none().then_some(label),
								date_uploaded: text(&link, ".card-ch-time")
									.and_then(|t| parse_relative_date(&t, now)),
								..Default::default()
							},
						})
					})
					.collect();
				if entries.is_empty() {
					continue;
				}
				HomeComponentValue::MangaChapterList {
					entries,
					page_size: Some(5),
					listing,
				}
			}
			_ => {
				let entries: Vec<_> = elements.iter().filter_map(card).map(Link::from).collect();
				if entries.is_empty() {
					continue;
				}
				if matches!(id, "yours" | "taste" | "finished") {
					HomeComponentValue::MangaList {
						ranking: true,
						page_size: Some(3),
						entries,
						listing,
					}
				} else {
					HomeComponentValue::Scroller { entries, listing }
				}
			}
		};
		components.push(HomeComponent {
			title,
			subtitle: None,
			value,
		});
	}
	HomeLayout { components }
}

fn browse_rows(shown: Option<&[String]>, prefs: &Catalogue) -> Result<HomeLayout> {
	let year = current_year();
	let rows: Vec<(&str, String, String)> = [
		(
			"popular",
			"Popular This Week".into(),
			"/browse?sort=popular".into(),
		),
		(
			"new",
			format!("New in {year}"),
			format!("/browse?from={year}&sort=updated"),
		),
		(
			"latest",
			"Latest Updates".into(),
			"/browse?sort=updated".into(),
		),
		(
			"manhwa",
			"Manhwa".into(),
			"/browse?kind=manhwa&sort=updated".into(),
		),
		(
			"manga",
			"Manga".into(),
			"/browse?kind=manga&sort=updated".into(),
		),
		(
			"manhua",
			"Manhua".into(),
			"/browse?kind=manhua&sort=updated".into(),
		),
	]
	.into_iter()
	.filter(|(id, ..)| {
		shown.is_none_or(|shown| shown.iter().any(|value| value == id))
			&& (!CONTENT_TYPES.contains(id) || prefs.types.iter().any(|kind| kind == id))
	})
	.collect();
	let mut pending = Vec::new();
	for row in rows {
		let view = (row.0 == "popular").then_some("list");
		if let Some(request) = browse_request(None, 1, link_filters(&row.2)?, prefs.clone(), view)?
		{
			pending.push((row, request));
		}
	}
	let (rows, requests): (Vec<_>, Vec<_>) = pending.into_iter().unzip();
	let mut components = Vec::new();
	let mut failure = None;
	for ((id, title, link), response) in rows.into_iter().zip(Request::send_all(requests)) {
		let entries = match response.map_err(Into::into).and_then(|response| {
			if id == "popular" {
				browse_table(&response)
			} else {
				browse_page(&response).map(|page| page.entries)
			}
		}) {
			Ok(entries) if !entries.is_empty() => entries,
			Ok(_) => continue,
			Err(error) => {
				failure = Some(error);
				continue;
			}
		};
		let value = if id == "popular" {
			HomeComponentValue::BigScroller {
				entries,
				auto_scroll_interval: Some(5.0),
			}
		} else {
			HomeComponentValue::Scroller {
				entries: entries.into_iter().map(Link::from).collect(),
				listing: Some(Listing {
					id: link,
					name: title.clone(),
					..Default::default()
				}),
			}
		};
		components.push(HomeComponent {
			title: Some(title),
			subtitle: None,
			value,
		});
	}
	match failure {
		Some(error) if components.is_empty() => Err(error),
		_ => Ok(HomeLayout { components }),
	}
}

impl Home for Webnex {
	fn get_home(&self) -> Result<HomeLayout> {
		let shown = settings::home_sections();
		if shown.as_ref().is_some_and(|shown| shown.is_empty()) {
			return Ok(HomeLayout::default());
		}
		let prefs = Catalogue::load();
		let Some(max) = prefs.ceiling().filter(|_| !prefs.types.is_empty()) else {
			return Ok(HomeLayout::default());
		};
		// Home supports a rating ceiling; browse also supports individual ratings.
		if CONTENT_RATINGS
			.iter()
			.take_while(|s| **s != max)
			.any(|r| !prefs.ratings.iter().any(|v| v == r))
		{
			return browse_rows(shown.as_deref(), &prefs);
		}
		let mut requests = vec![Request::get(BASE_URL)?.header("Cookie", &prefs.cookie(max))];
		if shown
			.as_ref()
			.is_none_or(|shown| shown.iter().any(|id| id == "popular"))
		{
			requests.extend(browse_request(
				None,
				1,
				link_filters("/browse?sort=popular")?,
				prefs,
				Some("list"),
			)?);
		}
		let mut responses = Request::send_all(requests).into_iter();
		let doc = parse_response(
			&responses
				.next()
				.ok_or_else(|| error!("Webnex did not answer"))??,
		)?;
		let mut home = layout(&doc, current_date(), shown.as_deref());
		if home.components.is_empty() && doc.select_first(".home-section").is_none() {
			bail!("Webnex did not return the home page");
		}
		// Shelf cards omit genres; browse's popular table is the same weekly chart with them.
		if let Some(Ok(mut table)) = responses
			.next()
			.map(|response| response.map_err(Into::into).and_then(|r| browse_table(&r)))
			&& let Some(HomeComponentValue::BigScroller { entries, .. }) =
				home.components.first_mut().map(|c| &mut c.value)
		{
			for manga in entries {
				if let Some(index) = table.iter().position(|row| row.key == manga.key) {
					manga.tags = table.swap_remove(index).tags;
				}
			}
		}
		Ok(home)
	}
}
