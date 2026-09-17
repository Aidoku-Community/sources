use crate::{
	XComic,
	graphql::{
		BrowseParams, HOME_LATEST_SIZE, PAGE_SIZE, browse_request, latest_uploads_request,
		parse_browse, parse_latest_uploads, parse_recently_added, parse_titles, random_request,
		recently_added_request, scroller_request,
	},
	helpers::{chapter_from_data, manga_from_data, team_of},
	models::ComicData,
	settings,
};
use aidoku::{
	BaseUrlProvider, Home, HomeComponent, HomeComponentValue, HomeLayout, Link, Listing,
	ListingKind, Manga, MangaWithChapter, Result,
	alloc::{Vec, vec},
	imports::net::{Request, RequestError, Response},
};

fn home_links(comics: Vec<ComicData>, base_url: &str) -> Vec<Link> {
	comics
		.into_iter()
		.filter_map(|comic| {
			let manga = manga_from_data(comic, base_url);
			manga.cover.is_some().then(|| Link::from(manga))
		})
		.collect()
}

impl Home for XComic {
	fn get_home(&self) -> Result<HomeLayout> {
		let base_url = self.get_base_url()?;
		let top_rated_params = BrowseParams::new("field_score", 1, 10);
		let most_followed_params = BrowseParams::new("field_follow", 1, PAGE_SIZE);
		let most_chapters_params = BrowseParams::new("field_chapter", 1, PAGE_SIZE);
		// Neither feed nor the random list takes a sort; these carry the reader's
		// content settings only.
		let feed_params = BrowseParams::new("field_update", 1, PAGE_SIZE);
		let responses: [core::result::Result<Response, RequestError>; 6] = Request::send_all([
			scroller_request(&base_url, &top_rated_params)?,
			browse_request(&base_url, &most_followed_params)?,
			random_request(&base_url)?,
			latest_uploads_request(&base_url, HOME_LATEST_SIZE)?,
			recently_added_request(&base_url)?,
			browse_request(&base_url, &most_chapters_params)?,
		])
		.try_into()
		.expect("requests vec length should be 6");
		let [
			top_rated,
			most_followed,
			random,
			latest,
			recently_added,
			most_chapters,
		] = responses;

		let top_rated: Vec<Manga> = parse_browse(top_rated?, &top_rated_params)?
			.0
			.into_iter()
			.filter_map(|comic| {
				let manga = manga_from_data(comic, &base_url);
				manga.cover.is_some().then_some(manga)
			})
			.collect();
		let random = home_links(parse_titles(random?, &feed_params)?.0, &base_url);
		let latest = parse_latest_uploads(latest?, &feed_params)?
			.into_iter()
			.filter_map(|(comic, chapter)| {
				let language = comic
					.translated_language
					.as_deref()
					.and_then(settings::normalize_language);
				let team = team_of(&comic);
				let chapter = chapter_from_data(
					chapter,
					&base_url,
					language.as_deref(),
					team.as_deref(),
					true,
				)?;
				let manga = manga_from_data(comic, &base_url);
				manga
					.cover
					.is_some()
					.then_some(MangaWithChapter { manga, chapter })
			})
			.collect();
		let recently_added = home_links(
			parse_recently_added(recently_added?, &feed_params)?,
			&base_url,
		);
		let most_followed = home_links(
			parse_browse(most_followed?, &most_followed_params)?.0,
			&base_url,
		);
		let most_chapters = home_links(
			parse_browse(most_chapters?, &most_chapters_params)?.0,
			&base_url,
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
					title: Some("Most Followed".into()),
					subtitle: None,
					value: HomeComponentValue::MangaList {
						ranking: true,
						page_size: Some(5),
						entries: most_followed,
						listing: Some(Listing {
							id: "field_follow".into(),
							name: "Most Followed".into(),
							kind: ListingKind::Default,
						}),
					},
				},
				HomeComponent {
					title: Some("Random Comics".into()),
					subtitle: None,
					value: HomeComponentValue::Scroller {
						entries: random,
						listing: None,
					},
				},
				HomeComponent {
					title: Some("Latest Update".into()),
					subtitle: None,
					value: HomeComponentValue::MangaChapterList {
						page_size: Some(5),
						entries: latest,
						listing: Some(Listing {
							id: "field_update".into(),
							name: "Latest Update".into(),
							kind: ListingKind::Default,
						}),
					},
				},
				HomeComponent {
					title: Some("Recently Added".into()),
					subtitle: None,
					value: HomeComponentValue::Scroller {
						entries: recently_added,
						listing: Some(Listing {
							id: "field_create".into(),
							name: "Recently Added".into(),
							kind: ListingKind::Default,
						}),
					},
				},
				HomeComponent {
					title: Some("Most Chapters".into()),
					subtitle: None,
					value: HomeComponentValue::Scroller {
						entries: most_chapters,
						listing: Some(Listing {
							id: "field_chapter".into(),
							name: "Most Chapters".into(),
							kind: ListingKind::Default,
						}),
					},
				},
			],
		})
	}
}
