//! Home page: a skeleton layout is sent first, then the WAP home page is
//! fetched and each parsed section fills its slot as a partial result. The
//! hot ranking additionally fetches every entry's detail page in parallel
//! to feed the big carousel (cover, author, description, tags).

use crate::Wenku8;
use crate::html::{NovelPage as _, document_gb, fetch_utf8_text};
use crate::net::{Url, cover_url, novel_url};
use aidoku::{
	Home, HomeComponent, HomeComponentValue, HomeLayout, HomePartialResult, Link, LinkValue,
	Listing, Manga, Result,
	alloc::{String, Vec, string::ToString, vec},
	imports::net::Request,
	imports::std::send_partial_result,
};

/// Number of detail pages fetched per parallel batch for the big carousel.
const BATCH_SIZE: usize = 6;

/// A recommendation section from the WAP homepage.
pub struct HomeSection {
	pub name: String,
	/// Toplist sort for the "view more" target.
	pub sort: String,
	/// (aid, title) pairs.
	pub entries: Vec<(String, String)>,
}

impl Home for Wenku8 {
	fn get_home(&self) -> Result<HomeLayout> {
		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: skeleton(),
		}));

		let html = fetch_utf8_text(&Url::WapHome)?;
		let sections = parse_home_sections(&html);
		if sections.is_empty() {
			return Ok(HomeLayout {
				components: Vec::new(),
			});
		}

		let mut hot: Option<HomeSection> = None;
		for section in sections {
			if section.sort == "dayvisit" {
				hot = Some(section);
			} else {
				send_partial_result(&HomePartialResult::Component(scroller_component(section)));
			}
		}

		if let Some(section) = hot {
			let component = hot_carousel(section);
			send_partial_result(&HomePartialResult::Component(component));
		}

		Ok(HomeLayout {
			components: Vec::new(),
		})
	}
}

/// An empty component per home slot, in display order. Partial results
/// replace them one by one as the responses arrive.
fn skeleton() -> Vec<HomeComponent> {
	vec![
		HomeComponent {
			title: Some("今日热榜".into()),
			subtitle: None,
			value: HomeComponentValue::BigScroller {
				entries: Vec::new(),
				auto_scroll_interval: Some(5.0),
			},
		},
		HomeComponent {
			title: Some("最近更新".into()),
			subtitle: None,
			value: HomeComponentValue::empty_scroller(),
		},
		HomeComponent {
			title: Some("新书风云榜".into()),
			subtitle: None,
			value: HomeComponentValue::empty_scroller(),
		},
	]
}

/// A cover scroller section with its "view more" listing attached.
fn scroller_component(section: HomeSection) -> HomeComponent {
	let HomeSection {
		name,
		sort,
		entries,
	} = section;
	HomeComponent {
		title: Some(name.clone()),
		subtitle: None,
		value: HomeComponentValue::Scroller {
			entries: entries
				.into_iter()
				.map(|(aid, title)| manga_link(aid, title))
				.collect(),
			listing: Some(Listing {
				id: sort,
				name,
				..Default::default()
			}),
		},
	}
}

/// The hot ranking as the big top carousel: fetch every entry's detail
/// page (in parallel batches) for author/description/tags. Entries whose
/// detail page fails degrade to cover + title.
fn hot_carousel(section: HomeSection) -> HomeComponent {
	let title = section.name;
	let entries: Vec<Manga> = section
		.entries
		.chunks(BATCH_SIZE)
		.flat_map(|chunk| {
			let requests: Vec<Request> = chunk
				.iter()
				.filter_map(|(aid, _)| Url::novel(aid).request().ok())
				.collect();
			// send_all preserves request order, so zip responses back with
			// their section entries.
			Request::send_all(requests)
				.into_iter()
				.zip(chunk.iter())
				.map(|(result, (aid, fallback_title))| {
					let cover = cover_url(aid);
					let url = novel_url(aid);
					let mut manga = Manga {
						key: aid.clone(),
						..Default::default()
					};
					if let Ok(response) = result
						&& let Ok(document) = document_gb(response)
					{
						let _ = document.update_details(&mut manga);
					}
					if manga.title.is_empty() {
						manga.title = fallback_title.clone();
					}
					if manga.cover.is_none() {
						manga.cover = Some(cover);
					}
					if manga.url.is_none() {
						manga.url = Some(url);
					}
					manga
				})
				.collect::<Vec<_>>()
		})
		.collect();

	HomeComponent {
		title: Some(title),
		subtitle: None,
		value: HomeComponentValue::BigScroller {
			entries,
			auto_scroll_interval: Some(5.0),
		},
	}
}

/// Parse the WAP homepage into recommendation sections.
///
/// The page is a flat run of `<br/>`-separated lines: a section header line
/// like `【今日热榜】<a href="...toplist.php?sort=dayvisit">更多..</a>`
/// followed by one `<a href="...articleinfo.php?id=N">title</a>` per line.
fn parse_home_sections(html: &str) -> Vec<HomeSection> {
	let mut sections: Vec<HomeSection> = vec![];
	for segment in html.split("<br") {
		let Some(sort_pos) = segment.find("toplist.php?sort=") else {
			// Novel entry line belonging to the current section.
			let Ok(doc) = Html::parse(segment) else {
				continue;
			};
			let Some(anchors) = doc.select("a[href*='articleinfo.php']") else {
				continue;
			};
			for anchor in anchors {
				let Some(href) = anchor.attr("href") else {
					continue;
				};
				let Some(id_pos) = href.find("id=") else {
					continue;
				};
				let aid: String = href[id_pos + 3..]
					.chars()
					.take_while(|c| c.is_ascii_digit())
					.collect();
				let title = anchor.text().unwrap_or_default().trim().to_string();
				if let (Some(section), false) =
					(sections.last_mut(), aid.is_empty() || title.is_empty())
				{
					section.entries.push((aid, title));
				}
			}
			continue;
		};
		let sort: String = segment[sort_pos + "toplist.php?sort=".len()..]
			.chars()
			.take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
			.collect();
		// The header text precedes the first tag, e.g. `【今日热榜】`.
		let name = segment
			.split('<')
			.next()
			.unwrap_or("")
			.chars()
			.filter(|c| !c.is_whitespace() && *c != '/' && *c != '>')
			.collect::<String>()
			.trim_matches(|c| c == '【' || c == '】')
			.to_string();
		sections.push(HomeSection {
			name,
			sort,
			entries: vec![],
		});
	}
	sections.retain(|s| !s.entries.is_empty());
	sections
}

/// Wrap a series entry as a home link; tapping it opens the series by key.
fn manga_link(aid: String, title: String) -> Link {
	let cover = cover_url(&aid);
	Link {
		title,
		subtitle: None,
		image_url: Some(cover),
		value: Some(LinkValue::Manga(Manga {
			key: aid,
			..Default::default()
		})),
	}
}

use aidoku::imports::html::Html;
