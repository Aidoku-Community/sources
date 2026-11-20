use crate::{
	Params,
	api::SearchParams,
	helpers::{
		checked, date, excluded_slugs, link_keys, novel_title, parse, rating, request, series_key,
		site_headers, viewer,
	},
	models::{MangaFeed, PopularChapters, Title, Updates},
	settings,
};
use aidoku::{
	AidokuError, Chapter, FilterValue, HomeComponent, HomeComponentValue, HomeLayout,
	HomePartialResult, Link, Manga, MangaWithChapter, Result,
	alloc::{
		String, Vec,
		collections::{BTreeMap, BTreeSet},
		vec,
	},
	imports::{
		html::Document,
		net::{Request, Response},
		std::send_partial_result,
	},
	prelude::*,
};

#[derive(Clone, Copy)]
pub enum Section {
	Featured,
	Ranking,
	Recent,
	PopularChapters,
	Updates,
	Followed,
	Popular,
	Novels,
	LatestNovels,
	HighScore,
}

impl Section {
	// The site's hero has no heading
	pub fn title(self) -> Option<&'static str> {
		Some(match self {
			Self::Featured => return None,
			Self::Ranking => "Most Followed New Manga (1 Month)",
			Self::Recent => "Recently Added",
			Self::PopularChapters => "Popular Chapters (This Week)",
			Self::Updates => "Latest Updates",
			Self::Followed => "Most Followed Manga (This Week)",
			Self::Popular => "Popular Manga (This Week)",
			Self::Novels => "Recently Added Novels",
			Self::LatestNovels => "Latest Novels",
			Self::HighScore => "High Score Manga",
		})
	}

	// Rows without a feed are read from the home page itself
	fn path(self) -> Option<&'static str> {
		match self {
			Self::Ranking => Some("/auth/most-followed-new-manga?period=1m&limit=15"),
			Self::PopularChapters => Some("/auth/popular-chapters?period=week&limit=15"),
			Self::Updates => Some("/wp-json/manga/v1/latest-chapters?page=1"),
			Self::Followed => Some("/auth/most-followed-manga?period=week&limit=15"),
			Self::Popular => Some("/wp-json/manga/v1/popular?period=week&number=15"),
			Self::HighScore => Some("/wp-json/manga/v1/highscore?type=all&number=15"),
			Self::LatestNovels => Some("/wp-json/manga/v1/load"),
			Self::Featured | Self::Recent | Self::Novels => None,
		}
	}
}

pub(crate) enum Data {
	Featured(Vec<Manga>),
	Titles(Vec<Title>),
	Chapters(Vec<MangaWithChapter>),
}

pub fn parse_featured(document: &Document, excluded_types: &[String]) -> Vec<Manga> {
	document
		.select(".carousel-slide")
		.into_iter()
		.flatten()
		.filter_map(|slide| {
			let link = slide.select_first("h2 a")?;
			let url = link.attr("href")?;
			let key = series_key(&url)?.into();
			let title = link.text().filter(|s| !s.is_empty())?;
			let kind = slide
				.select_first("span.uppercase")
				.and_then(|el| el.text());
			if kind
				.as_ref()
				.is_some_and(|kind| excluded_types.contains(kind))
			{
				return None;
			}
			let mut tags = slide
				.select("div.hidden span")
				.into_iter()
				.flatten()
				.filter_map(|el| el.text())
				.collect::<Vec<_>>();
			tags.sort_unstable();
			tags.dedup();
			Some(Manga {
				key,
				title,
				cover: slide.select_first("a img").and_then(|el| el.attr("src")),
				description: slide.select_first("p").and_then(|el| el.text()),
				content_rating: rating(&tags),
				tags: (!tags.is_empty()).then_some(tags),
				viewer: kind.as_deref().map(viewer).unwrap_or_default(),
				..Default::default()
			})
		})
		.collect()
}

pub fn parse_recent(document: &Document) -> Vec<Title> {
	document
		.select("#manga-grid > .group")
		.into_iter()
		.flatten()
		.filter_map(|card| {
			let image = card.select_first("img")?;
			Some(Title {
				title: image.attr("alt").filter(|s| !s.trim().is_empty())?,
				cover: image.attr("src")?,
				url: card.select_first("a[href*='/manga/']")?.attr("href")?,
				..Default::default()
			})
		})
		.collect()
}

pub fn parse_novels(document: &Document) -> Vec<MangaWithChapter> {
	document
		.select("#novels-slider-container .group")
		.into_iter()
		.flatten()
		.filter_map(|card| {
			let image = card.select_first("img")?;
			let title = image.attr("alt").filter(|s| !s.trim().is_empty())?;

			let chapter_link = card.select_first("a[href*='/read/']")?;
			let chapter_url = chapter_link.attr("href")?;
			let (key, Some(chapter_key)) = link_keys(&chapter_url)? else {
				return None;
			};
			Some(MangaWithChapter {
				manga: Manga {
					key,
					title: novel_title(title),
					cover: image.attr("src"),
					..Default::default()
				},
				chapter: Chapter {
					key: chapter_key,
					chapter_number: chapter_link
						.text()?
						.trim_start_matches("Ch.")
						.trim()
						.parse()
						.ok(),
					date_uploaded: card
						.select_first("span")
						.and_then(|el| el.text())
						.and_then(|text| date(&text)),
					url: Some(chapter_url),
					..Default::default()
				},
			})
		})
		.collect()
}

fn parse_feed(section: Section, response: Response) -> Result<Data> {
	match section {
		Section::Ranking | Section::Followed => {
			let feed: MangaFeed = parse(response)?;
			if !feed.success {
				bail!("Could not load followed manga");
			}
			Ok(Data::Titles(feed.manga))
		}
		Section::PopularChapters => {
			let feed: PopularChapters = parse(response)?;
			if !feed.success {
				bail!("Could not load popular chapters");
			}
			Ok(Data::Titles(feed.chapters))
		}
		Section::Updates => {
			let feed: Updates = parse(response)?;
			if !feed.success {
				bail!("Could not load latest updates");
			}
			Ok(Data::Titles(feed.data))
		}
		_ => Ok(Data::Titles(parse(response)?)),
	}
}

pub(crate) fn component(
	section: Section,
	data: Data,
	excluded_types: &[String],
	blocked: &BTreeSet<String>,
) -> HomeComponent {
	let value = match data {
		Data::Featured(entries) => HomeComponentValue::BigScroller {
			entries: entries
				.into_iter()
				.filter(|m| !blocked.contains(&m.key))
				.collect(),
			auto_scroll_interval: Some(5.0),
		},
		Data::Chapters(entries) => HomeComponentValue::MangaChapterList {
			entries: entries
				.into_iter()
				.filter(|m| !blocked.contains(&m.manga.key))
				.collect(),
			page_size: Some(4),
			listing: None,
		},
		Data::Titles(titles) => {
			let mut seen = BTreeSet::new();
			let titles = titles.into_iter().filter(|t| {
				!excluded_types.contains(&t.manga_type)
					&& t.key()
						.is_some_and(|key| !blocked.contains(key) && seen.insert(String::from(key)))
			});
			match section {
				Section::Updates => HomeComponentValue::MangaChapterList {
					entries: titles.filter_map(|t| t.into_update().ok()).collect(),
					page_size: Some(4),
					listing: None,
				},
				Section::Ranking => HomeComponentValue::MangaList {
					entries: titles
						.filter_map(|t| Manga::try_from(t).ok())
						.map(Link::from)
						.collect(),
					page_size: Some(3),
					ranking: true,
					listing: None,
				},
				_ => HomeComponentValue::Scroller {
					entries: titles
						.filter_map(|t| Manga::try_from(t).ok())
						.map(Link::from)
						.collect(),
					listing: None,
				},
			}
		}
	};
	HomeComponent {
		title: section.title().map(Into::into),
		value,
		..Default::default()
	}
}

pub fn get_home(params: &Params) -> Result<HomeLayout> {
	let base_url = params.base_url.as_ref();
	let types = settings::excluded_types();
	let novels = !types.iter().any(|kind| kind == "Novel");
	let sections = params
		.sections
		.iter()
		.copied()
		.filter(|section| novels || !matches!(section, Section::Novels | Section::LatestNovels))
		.collect::<Vec<_>>();
	send_partial_result(&HomePartialResult::Layout(HomeLayout {
		components: sections
			.iter()
			.map(|section| HomeComponent {
				title: section.title().map(Into::into),
				value: match section {
					Section::Featured => HomeComponentValue::empty_big_scroller(),
					Section::Ranking => HomeComponentValue::empty_manga_list(),
					Section::Updates | Section::Novels => {
						HomeComponentValue::empty_manga_chapter_list()
					}
					_ => HomeComponentValue::empty_scroller(),
				},
				..Default::default()
			})
			.collect(),
	}));
	let mut requests = vec![request(base_url, &format!("{base_url}/home"))?];
	for section in &sections {
		if let Some(path) = section.path() {
			let url = format!("{base_url}{path}");
			requests.push(match section {
				Section::Updates => site_headers(Request::post(url)?, base_url)
					.header("Content-Type", "application/json"),
				Section::LatestNovels => SearchParams::new(
					None,
					1,
					vec![FilterValue::MultiSelect {
						id: "type".into(),
						included: vec!["Novel".into()],
						excluded: Vec::new(),
					}],
				)?
				.request(base_url)?,
				_ => request(base_url, &url)?,
			});
		}
	}
	let mut responses = Request::send_all(requests).into_iter();
	let mut failure: Option<AidokuError> = None;
	let document = responses.next().and_then(|response| {
		response
			.map_err(AidokuError::from)
			.and_then(|response| Ok(checked(response)?.get_html()?))
			.map_err(|error| failure = Some(error))
			.ok()
	});
	let mut data = Vec::new();
	for section in sections {
		let result = match (section.path(), &document) {
			(Some(_), _) => match responses.next() {
				Some(response) => response
					.map_err(AidokuError::from)
					.and_then(|response| parse_feed(section, response)),
				None => continue,
			},
			(None, Some(document)) => Ok(match section {
				Section::Featured => Data::Featured(parse_featured(document, &types)),
				Section::Recent => Data::Titles(parse_recent(document)),
				_ => Data::Chapters(parse_novels(document)),
			}),
			(None, None) => continue,
		};
		match result {
			Ok(value) => data.push((section, value)),
			Err(error) => failure = Some(error),
		}
	}
	if data.is_empty() {
		return Err(failure.unwrap_or_else(|| error!("No home sections")));
	}
	let kinds = data
		.iter()
		.filter_map(|(_, data)| match data {
			Data::Titles(titles) => Some(titles),
			_ => None,
		})
		.flatten()
		.filter(|t| !t.manga_type.is_empty())
		.filter_map(|t| Some((String::from(t.key()?), t.manga_type.clone())))
		.collect::<BTreeMap<_, _>>();
	for (_, row) in &mut data {
		if let Data::Titles(titles) = row {
			for title in titles {
				if title.manga_type.is_empty()
					&& let Some(kind) = title.key().and_then(|key| kinds.get(key))
				{
					title.manga_type = kind.clone();
				}
			}
		}
	}
	let keys = data.iter().flat_map(|(_, data)| match data {
		Data::Featured(entries) => entries.iter().map(|m| m.key.as_str()).collect::<Vec<_>>(),
		Data::Titles(entries) => entries.iter().filter_map(Title::key).collect(),
		Data::Chapters(entries) => entries.iter().map(|m| m.manga.key.as_str()).collect(),
	});
	let blocked = excluded_slugs(base_url, keys, &settings::excluded_tags())?;
	Ok(HomeLayout {
		components: data
			.into_iter()
			.map(|(section, data)| component(section, data, &types, &blocked))
			.collect(),
	})
}
