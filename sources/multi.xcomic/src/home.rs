use crate::{
	XComic,
	graphql::{
		BrowseParams, PAGE_SIZE, browse_request, latest_uploads_request, parse_browse,
		parse_latest_uploads, parse_recently_added, recently_added_request, scroller_request,
	},
	helpers::{chapter_from_data, manga_from_data},
	models::{ComicData, LatestEntry},
	settings,
};
use aidoku::{
	BaseUrlProvider, Home, HomeComponent, HomeComponentValue, HomeLayout, Link, Listing,
	ListingKind, Manga, MangaWithChapter, Result,
	alloc::{Vec, vec},
	imports::net::{Request, RequestError, Response},
};

// Lets a section header open the matching `source.json` listing.
fn listing(id: &str, name: &str) -> Option<Listing> {
	Some(Listing {
		id: id.into(),
		name: name.into(),
		kind: ListingKind::Default,
	})
}

// Home sections only render covers, so coverless entries are dropped.
fn visible(comic: ComicData, base_url: &str) -> Option<Manga> {
	let manga = manga_from_data(comic, base_url);
	manga.cover.is_some().then_some(manga)
}

fn links(comics: Vec<ComicData>, base_url: &str, limit: usize) -> Vec<Link> {
	comics
		.into_iter()
		.filter_map(|comic| Some(Link::from(visible(comic, base_url)?)))
		.take(limit)
		.collect()
}

/// Pairs each upload with its chapter so the app can show a relative timestamp.
fn chapter_entries(items: Vec<LatestEntry>, base_url: &str, limit: usize) -> Vec<MangaWithChapter> {
	items
		.into_iter()
		.filter_map(|(comic, chapter)| {
			let language = comic
				.translated_language
				.as_deref()
				.and_then(settings::normalize_language);
			let chapter = chapter_from_data(chapter?, base_url, language.as_deref(), true)?;
			Some(MangaWithChapter {
				manga: visible(comic, base_url)?,
				chapter,
			})
		})
		.take(limit)
		.collect()
}

impl Home for XComic {
	fn get_home(&self) -> Result<HomeLayout> {
		let base_url = self.get_base_url()?;
		let top_rated_params = BrowseParams::new("field_score", 1)?;
		let most_viewed_params = BrowseParams::new("views_d030", 1)?;
		let most_chapters_params = BrowseParams::new("field_chapter", 1)?;
		// Neither feed takes a sort; these carry the reader's content settings only.
		let feed_params = BrowseParams::new("field_update", 1)?;
		let responses: [core::result::Result<Response, RequestError>; 5] = Request::send_all([
			scroller_request(&base_url, &top_rated_params)?,
			browse_request(&base_url, &most_viewed_params)?,
			latest_uploads_request(&base_url, None)?,
			recently_added_request(&base_url)?,
			browse_request(&base_url, &most_chapters_params)?,
		])
		.try_into()
		.expect("requests vec length should be 5");
		let [
			top_rated,
			most_viewed,
			latest,
			recently_added,
			most_chapters,
		] = responses;

		let page = PAGE_SIZE as usize;
		let top_rated: Vec<Manga> = parse_browse(top_rated?, &top_rated_params)?
			.0
			.into_iter()
			.filter_map(|comic| visible(comic, &base_url))
			.take(10)
			.collect();
		let most_viewed = links(
			parse_browse(most_viewed?, &most_viewed_params)?.0,
			&base_url,
			usize::MAX,
		);
		let latest = chapter_entries(
			parse_latest_uploads(latest?, &feed_params)?.0,
			&base_url,
			page,
		);
		let recently_added = links(
			parse_recently_added(recently_added?, &feed_params)?,
			&base_url,
			page,
		);
		let most_chapters = links(
			parse_browse(most_chapters?, &most_chapters_params)?.0,
			&base_url,
			usize::MAX,
		);

		Ok(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some("Top Rated".into()),
					subtitle: None,
					value: HomeComponentValue::BigScroller {
						entries: top_rated,
						auto_scroll_interval: Some(6.0),
					},
				},
				HomeComponent {
					title: Some("Most Viewed (30 Days)".into()),
					subtitle: None,
					value: HomeComponentValue::MangaList {
						ranking: true,
						page_size: Some(5),
						entries: most_viewed,
						listing: listing("views_d030", "Most Viewed (30 Days)"),
					},
				},
				HomeComponent {
					title: Some("Latest Update".into()),
					subtitle: None,
					value: HomeComponentValue::MangaChapterList {
						page_size: Some(5),
						entries: latest,
						listing: listing("field_update", "Latest Update"),
					},
				},
				HomeComponent {
					title: Some("Recently Added".into()),
					subtitle: None,
					value: HomeComponentValue::Scroller {
						entries: recently_added,
						listing: listing("field_create", "Recently Added"),
					},
				},
				HomeComponent {
					title: Some("Most Chapters".into()),
					subtitle: None,
					value: HomeComponentValue::MangaList {
						ranking: true,
						page_size: Some(5),
						entries: most_chapters,
						listing: listing("field_chapter", "Most Chapters"),
					},
				},
			],
		})
	}
}
