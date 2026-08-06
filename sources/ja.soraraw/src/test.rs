use super::*;
use aidoku::{AidokuError, ContentRating, FilterKind, MangaStatus, Viewer};
use aidoku_test::aidoku_test;

/// "Majo to Youhei", a long running series used to check parsing against.
const MANGA_KEY: &str = "majo-to-youhei-57539";
/// "Mattan Heishi ga Kunshu ni Naru made", a korean webtoon carrying the overseas genre.
const WEBTOON_KEY: &str = "mattan-heishi-ga-kunshu-ni-naru-made-7194";
/// "Blue Giant Momentum", ordinary manga the site marks as `vertical`. Reported as opening in a
/// continuous scroll, which is what picking the reader from `mode` used to do to it.
const PAGED_VERTICAL_KEY: &str = "blue-giant-momentum-buruu-jaianto-momentamu-60652";
/// "Hard Worker Nakata", also marked `vertical` while holding page-shaped art.
const ADULT_VERTICAL_KEY: &str = "haadowaakaa-nakata-740";
/// "Tonari no Kurokawa-san", which holds one of the chapters stored as jpg.
const JPG_MANGA_KEY: &str = "my-neighbor-ms-kurokawa-tonari-no-kurokawa-san-1";
const JPG_CHAPTER_KEY: &str = "1/786104";
/// "Kobayashi-san Chi no Maid Dragon", used to check that searching finds a series by both its
/// japanese and its english title.
const SEARCHED_KEY: &str = "kobayashi-san-chino-meidoragon-57605";

fn listing(id: &str) -> Listing {
	Listing {
		id: String::from(id),
		..Default::default()
	}
}

fn resolves(url: &str) -> bool {
	Request::head(url)
		.and_then(|request| request.send())
		.map(|response| response.status_code() == 200)
		.unwrap_or(false)
}

fn titles(entries: &[Manga]) -> Vec<&String> {
	entries.iter().map(|manga| &manga.title).collect()
}

fn page_urls(pages: &[Page]) -> Vec<&String> {
	pages
		.iter()
		.map(|page| {
			let PageContent::Url(url, _) = &page.content else {
				panic!("expected a page url");
			};
			url
		})
		.collect()
}

#[aidoku_test]
fn test_listings() {
	for id in ["newest", "hot", "trending"] {
		let result = Soraraw.get_manga_list(listing(id), 1).expect("listing");
		assert!(!result.entries.is_empty(), "{id} returned no entries");

		let entry = &result.entries[0];
		assert!(!entry.key.is_empty());
		assert!(!entry.title.is_empty());
		assert!(
			entry
				.cover
				.as_ref()
				.is_some_and(|cover| cover.starts_with("http")),
			"{id} entry has no absolute cover"
		);
		// listings carry the adult flag, so this one doesn't have to wait for the details
		// request; `viewer` does, since it is picked from genres the listings don't carry
		assert_ne!(
			entry.content_rating,
			ContentRating::Unknown,
			"{id} entry has no content rating"
		);
	}
}

// only the paginated listing walks pages; the other two hand out a single batch
#[aidoku_test]
fn test_listing_pagination() {
	let first = Soraraw
		.get_manga_list(listing("newest"), 1)
		.expect("page 1");
	assert!(first.has_next_page);

	let second = Soraraw
		.get_manga_list(listing("newest"), 2)
		.expect("page 2");
	assert!(!second.entries.is_empty());
	assert_ne!(first.entries[0].key, second.entries[0].key);

	let hot = Soraraw.get_manga_list(listing("hot"), 1).expect("hot");
	assert!(!hot.has_next_page);
}

// searching walks the catalogue dump, because the site's own "/search" page answers with the same
// fixed batch no matter what it is asked for. the series below is looked up by its japanese title
// and by its romanised alternative one, which has to match regardless of case
#[aidoku_test]
fn test_search() {
	for query in ["小林さんちのメイドラゴン", "miss kobayashi"] {
		let result = Soraraw
			.get_search_manga_list(Some(String::from(query)), 1, Vec::new())
			.expect("search");
		let entry = result
			.entries
			.iter()
			.find(|manga| manga.key == SEARCHED_KEY)
			.unwrap_or_else(|| panic!("{query} matched {:?}", titles(&result.entries)));

		assert_eq!(entry.title, "小林さんちのメイドラゴン");
		assert_eq!(entry.content_rating, ContentRating::NSFW);
		// catalogue entries name their cover field differently, so it is easy to lose
		assert!(
			entry
				.cover
				.as_ref()
				.is_some_and(|cover| cover.starts_with("http")),
			"{query} matched an entry without an absolute cover"
		);
		// every match is returned at once, so there is no page to follow
		assert!(!result.has_next_page);
	}
}

// "supportsAuthorSearch" puts an author field in the search filters rather than folding the name
// into the query, so the walk has to narrow to the author of an entry when that field arrives
#[aidoku_test]
fn test_search_by_author() {
	let filters = vec![FilterValue::Text {
		id: String::from("author"),
		value: String::from("伊藤京介"),
	}];
	let result = Soraraw
		.get_search_manga_list(None, 1, filters)
		.expect("author search");

	assert!(
		result
			.entries
			.iter()
			.any(|manga| manga.key == JPG_MANGA_KEY),
		"{:?}",
		titles(&result.entries)
	);
	assert!(!result.has_next_page);
}

// the author field narrows to the author column alone. the walk above cannot tell that apart from
// one that ran over every field, since the plain query matches the author too — so the narrowing
// is pinned here instead, where a title that only the query should match has to be rejected
#[aidoku_test]
fn test_matches_author() {
	let entry = serde_json::from_str::<CatalogueEntry>(
		r#"{"name":"となりの黒川さん","slug":"x","author":"伊藤京介"}"#,
	)
	.expect("catalogue entry");
	assert!(entry.matches_author("伊藤京介"));
	assert!(!entry.matches_author("となりの黒川さん"));
	// the plain query keeps running over the title as well as the author
	assert!(entry.matches("となりの黒川さん"));
	assert!(entry.matches("伊藤京介"));

	// about half the catalogue carries no author at all, which matches nothing rather than everything
	let anonymous =
		serde_json::from_str::<CatalogueEntry>(r#"{"name":"x","slug":"y"}"#).expect("entry");
	assert!(!anonymous.matches_author("伊藤京介"));
}

#[aidoku_test]
fn test_genre_filter() {
	let filters = vec![FilterValue::Select {
		id: String::from("genre"),
		value: String::from("akushon"),
	}];
	let result = Soraraw
		.get_search_manga_list(None, 1, filters)
		.expect("filtered list");
	assert!(!result.entries.is_empty());
	assert!(result.has_next_page);

	// an empty selection is the "All" option, which has to fall back to the plain listing
	let cleared = vec![FilterValue::Select {
		id: String::from("genre"),
		value: String::new(),
	}];
	let result = Soraraw
		.get_search_manga_list(None, 1, cleared)
		.expect("cleared filter");
	assert!(!result.entries.is_empty());
}

#[aidoku_test]
fn test_dynamic_filters() {
	let filters = Soraraw.get_dynamic_filters().expect("dynamic filters");
	assert_eq!(filters.len(), 1);

	let FilterKind::Select { options, ids, .. } = &filters[0].kind else {
		panic!("expected a select filter");
	};
	// the site listed over 1800 genres at the time of writing, so the cap is what decides the
	// count; the lower bound only guards against the list coming back empty or broken
	assert!(options.len() > 1, "got {} options", options.len());
	assert!(options.len() <= GENRE_LIMIT + 1, "the cap is not applied");
	let ids = ids.as_ref().expect("genre ids");
	assert_eq!(ids.len(), options.len());
	// the first option clears the filter, every other one has to name a genre
	assert!(ids[0].is_empty());
	assert!(ids[1..].iter().all(|id| !id.is_empty()));
}

#[aidoku_test]
fn test_manga_details() {
	let manga = Manga {
		key: String::from(MANGA_KEY),
		..Default::default()
	};
	let manga = Soraraw
		.get_manga_update(manga, true, true)
		.expect("manga details");

	assert_eq!(manga.title, "魔女と傭兵");
	assert_eq!(
		manga.url.as_deref(),
		Some("https://soraraw.com/manga/majo-to-youhei-57539")
	);
	assert!(manga.cover.is_some_and(|cover| cover.starts_with("http")));
	// the author field holds several names separated by commas
	assert!(manga.authors.is_some_and(|authors| authors.len() > 1));
	assert!(manga.tags.is_some_and(|tags| !tags.is_empty()));
	assert_eq!(manga.status, MangaStatus::Ongoing);
	assert_eq!(manga.viewer, Viewer::RightToLeft);
	assert_eq!(manga.content_rating, ContentRating::Safe);
	// the synopsis is stored as an editor document, which has to come out as plain text
	let description = manga.description.expect("description");
	assert!(!description.contains('<'), "{description}");
	assert!(!description.contains("blocks"), "{description}");

	let chapters = manga.chapters.expect("chapters");
	assert!(chapters.len() > 100, "got {} chapters", chapters.len());
	let chapter = &chapters[0];
	assert!(chapter.key.contains('/'));
	assert!(chapter.chapter_number.is_some());
	assert!(chapter.url.as_deref().is_some_and(|url| {
		url.starts_with("https://soraraw.com/manga/majo-to-youhei-57539/ch-")
	}));
	// language stays unset so the app's chapter language filter can't hide these
	assert_eq!(chapter.language, None);
	// decimal chapters exist and have to keep their number
	assert!(
		chapters
			.iter()
			.any(|chapter| chapter.chapter_number.is_some_and(|it| it.fract() != 0.0))
	);
	// date_uploaded isn't checked here: the test runner doesn't implement the quoting,
	// fractional seconds or ISO 8601 zones that DATE_FORMAT relies on, so it only ever
	// parses on device
}

// the reader is picked from genres rather than from the `mode` field: the site marks plenty of
// ordinary manga as `vertical`, and reading that flag as "webtoon" handed those to the continuous
// scroll reader, which ran every page of a chapter together
#[aidoku_test]
fn test_viewer_and_content_rating() {
	for (key, viewer, rating) in [
		// tagged with the overseas genre, so its panels are meant to run together
		(WEBTOON_KEY, Viewer::Webtoon, None),
		(PAGED_VERTICAL_KEY, Viewer::RightToLeft, None),
		// the adult flag has to reach the app regardless of which reader is picked. the two above
		// are left unasserted because what the site flags them as was never measured
		(
			ADULT_VERTICAL_KEY,
			Viewer::RightToLeft,
			Some(ContentRating::NSFW),
		),
	] {
		let manga = Manga {
			key: String::from(key),
			..Default::default()
		};
		let manga = Soraraw
			.get_manga_update(manga, true, false)
			.expect("details of a vertical entry");

		assert_eq!(manga.viewer, viewer, "{key}");
		if let Some(rating) = rating {
			assert_eq!(manga.content_rating, rating, "{key}");
		}
	}
}

#[aidoku_test]
fn test_page_list() {
	let manga = Manga {
		key: String::from(MANGA_KEY),
		..Default::default()
	};
	let mut manga = Soraraw
		.get_manga_update(manga, false, true)
		.expect("chapters");
	// taken rather than cloned; the page request doesn't read the chapter list back
	let chapter = manga
		.chapters
		.take()
		.expect("chapters")
		.into_iter()
		.find(|chapter| chapter.chapter_number == Some(1.0))
		.expect("chapter 1");

	let pages = Soraraw.get_page_list(manga, chapter).expect("page list");
	// chapter 1 of this series holds 72 pages, and can only ever gain them
	assert!(pages.len() >= 72, "got {} pages", pages.len());

	let urls = page_urls(&pages);
	for url in &urls {
		assert!(url.starts_with("https://lh"), "{url} is not absolute");
		assert!(url.ends_with(".webp"), "{url} is not a webp");
	}
	// page numbers are built rather than handed out, so they have to stay in order and unique
	assert!(
		urls[0].contains("/001_"),
		"{} is not the first page",
		urls[0]
	);
	assert!(
		urls[1].contains("/002_"),
		"{} is not the second page",
		urls[1]
	);

	// the urls are assembled from ids, so one has to be requested to prove the shape resolves
	assert!(resolves(urls[0]), "{} did not resolve", urls[0]);
	assert!(
		resolves(urls[urls.len() - 1]),
		"{} did not resolve",
		urls[urls.len() - 1]
	);
}

// at least chapters 66 to 70 of this series are each served as one image holding all 24 of
// their pages stacked on top of each other, 49152 pixels tall. The reader can't draw that and
// no cut a source can make reaches into it while Aidoku/AidokuRunner#3 stands, so the chapter
// has to fail with a reason rather than hand back a page that renders blank. Getting as far as
// the refusal also means the decrypted path resolved and its header read back
#[aidoku_test]
fn test_stacked_chapter_is_refused() {
	let manga = Manga {
		key: String::from(PAGED_VERTICAL_KEY),
		..Default::default()
	};
	let mut manga = Soraraw
		.get_manga_update(manga, false, true)
		.expect("chapters");
	let chapter = manga
		.chapters
		.take()
		.expect("chapters")
		.into_iter()
		.find(|chapter| chapter.chapter_number == Some(70.0))
		.expect("chapter 70");

	let Err(AidokuError::Message(reason)) = Soraraw.get_page_list(manga, chapter) else {
		panic!("an undrawable strip has to fail rather than hand back a blank page");
	};
	// the image measured 1450x49152 against the 1448x2048 the rest of the series holds
	assert!(reason.contains(".jpg"), "{reason}");
	assert!(reason.contains("49152"), "{reason}");
}

// stacked chapters are not one series' quirk: this one holds 21 pages in a single 800x24003
// jpg, so it has to be refused the same way. The chapter has to come from the chapter list
// rather than be built by hand — its url is where the key to the paths is read from
#[aidoku_test]
fn test_stacked_chapter_of_another_series_is_refused() {
	let manga = Manga {
		key: String::from(JPG_MANGA_KEY),
		..Default::default()
	};
	let mut manga = Soraraw
		.get_manga_update(manga, false, true)
		.expect("chapters");
	let chapter = manga
		.chapters
		.take()
		.expect("chapters")
		.into_iter()
		.find(|chapter| chapter.key == JPG_CHAPTER_KEY)
		.expect("the jpg chapter");

	let Err(AidokuError::Message(reason)) = Soraraw.get_page_list(manga, chapter) else {
		panic!("an undrawable strip has to fail rather than hand back a blank page");
	};
	assert!(reason.contains(".jpg"), "{reason}");
	assert!(reason.contains("24003"), "{reason}");
}

// a chapter numbered "74.2" has to survive the round trip into a page request
#[aidoku_test]
fn test_decimal_chapter_pages() {
	let manga = Manga {
		key: String::from(MANGA_KEY),
		..Default::default()
	};
	let mut manga = Soraraw
		.get_manga_update(manga, false, true)
		.expect("chapters");
	let chapter = manga
		.chapters
		.take()
		.expect("chapters")
		.into_iter()
		.find(|chapter| {
			chapter
				.chapter_number
				.is_some_and(|number| number.fract() != 0.0)
		})
		.expect("a decimal chapter");

	let pages = Soraraw
		.get_page_list(manga, chapter)
		.expect("decimal chapter pages");
	assert!(!pages.is_empty());
}

#[aidoku_test]
fn test_deep_link() {
	let manga = Soraraw
		.handle_deep_link(String::from(
			"https://soraraw.com/manga/majo-to-youhei-57539",
		))
		.expect("manga deep link");
	assert_eq!(
		manga,
		Some(DeepLinkResult::Manga {
			key: String::from(MANGA_KEY)
		})
	);

	// shared links carry tracking parameters the key must not pick up
	let shared = Soraraw
		.handle_deep_link(String::from(
			"https://soraraw.com/manga/majo-to-youhei-57539?utm_source=share",
		))
		.expect("shared manga deep link");
	assert_eq!(
		shared,
		Some(DeepLinkResult::Manga {
			key: String::from(MANGA_KEY)
		})
	);

	let chapter = Soraraw
		.handle_deep_link(String::from(
			"https://soraraw.com/manga/majo-to-youhei-57539/ch-1",
		))
		.expect("chapter deep link");
	assert_eq!(
		chapter,
		Some(DeepLinkResult::Chapter {
			manga_key: String::from(MANGA_KEY),
			key: String::from("57539/508048"),
		})
	);

	let unknown = Soraraw
		.handle_deep_link(String::from("https://soraraw.com/newest"))
		.expect("unknown deep link");
	assert_eq!(unknown, None);

	let foreign = Soraraw
		.handle_deep_link(String::from(
			"https://example.com/manga/majo-to-youhei-57539",
		))
		.expect("foreign deep link");
	assert_eq!(foreign, None);
}

// malformed keys can reach the source from a stale library entry, and have to fail loudly
#[aidoku_test]
fn test_malformed_chapter_key() {
	let chapter = Chapter {
		key: String::from("not-a-key"),
		..Default::default()
	};
	assert!(Soraraw.get_page_list(Manga::default(), chapter).is_err());

	let chapter = Chapter {
		key: String::from("57539/not-a-number"),
		..Default::default()
	};
	assert!(Soraraw.get_page_list(Manga::default(), chapter).is_err());
}

// the payload decoding is pure, so it can be checked without touching the network
#[aidoku_test]
fn test_deobfuscate() {
	// "[{\"id\":1,\"order\":1}]" xored with the key and encoded, padding left off
	let payload = "dB1XKg97VUQNA05dAhAxSWNeCHw";
	let json = deobfuscate(payload, PAYLOAD_KEY).expect("decoded payload");
	assert_eq!(json, r#"[{"id":1,"order":1}]"#);

	// the endpoint gives the order as a number, but strings appear in the same shape of
	// payload elsewhere on the site, so both have to survive
	let images =
		serde_json::from_str::<Vec<PageImage>>(r#"[{"order":"12","b":"AA"}]"#).expect("text");
	assert_eq!(images[0].order.as_f32(), Some(12.0));

	assert_eq!(deobfuscate("*", PAYLOAD_KEY), None);
	assert_eq!(decode_base64("QUJD"), Some(Vec::from(*b"ABC")));
	assert_eq!(decode_base64("QUJD="), Some(Vec::from(*b"ABC")));
	assert_eq!(decode_base64("*"), None);
}

// the path of a page image is what the site encrypts, and building one out of the ids instead
// held only for part of it: "BLUE GIANT MOMENTUM" is served as jpg, not webp
#[aidoku_test]
fn test_decrypt_path() {
	// chapter 722455, whose uuid the chapter page carries
	let uuid = "003a8cfd16281b2b1d255d06524d8639ac1f7497533220824247f76eba48aeb9";
	let path = decrypt_path(
		"UQajtZjw1-nFt7IgA1ot0f9ahvQL5noTTVZv-2P0EASrNeOXH94Kyw",
		uuid,
		PATH_SECRET,
	);
	assert_eq!(path.as_deref(), Some("c722455/001_25706548.jpg"));

	// a uuid that isn't a 32 byte key, and a value too short to hold a counter and a path
	assert_eq!(
		decrypt_path("UQajtZjw1-nFt7IgA1ot0f9a", "00ff", PATH_SECRET),
		None
	);
	assert_eq!(decrypt_path("QUJD", uuid, PATH_SECRET), None);
}

// the header of a jpeg is what refusing an undrawable chapter rests on, and a frame header sits
// behind however many segments the encoder wrote ahead of it
#[aidoku_test]
fn test_jpeg_height() {
	let head = [
		0xFF, 0xD8, // start of image
		0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00, // an app segment to skip over
		0xFF, 0xC0, 0x00, 0x11, 0x08, 0xC0, 0x00, 0x05, 0xAA, // a frame of 1450 x 49152
	];
	assert_eq!(jpeg_height(&head), Some(49152));

	// anything that isn't a jpeg, and a header cut off ahead of the frame
	assert_eq!(jpeg_height(b"RIFF\0\0\0\0WEBPVP8 "), None);
	assert_eq!(jpeg_height(&head[..8]), None);
}
