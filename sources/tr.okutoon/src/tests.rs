use super::*;
use aidoku::{ContentRating, MangaStatus, Viewer, alloc::vec};
use aidoku_test::aidoku_test;

fn parse<T: DeserializeOwned>(json: &str) -> T {
	serde_json::from_str(json).expect("fixture")
}

#[aidoku_test]
fn catalog_page() {
	let list: SeriesList = parse(include_str!("../fixtures/list.json"));
	let result: MangaPageResult = list.into();
	assert_eq!(result.entries.len(), 2);
	assert!(result.has_next_page);
	let first = &result.entries[0];
	assert_eq!(first.key, "the-genius-midfielder-with-the-killer-pass");
	assert_eq!(first.title, "The Genius Midfielder with the Killer Pass");
	assert!(
		first
			.cover
			.as_deref()
			.is_some_and(|c| c.starts_with("https://cdn.okutoon.net/"))
	);
	// listings only carry the basics
	assert!(first.description.is_none() && first.tags.is_none());
}

#[aidoku_test]
fn listing_keeps_content_rating() {
	let series: Series = parse(r#"{"slug":"x","title":"X","categories":[{"name":"Ecchi"}]}"#);
	assert_eq!(
		series.into_basic_manga().content_rating,
		ContentRating::Suggestive
	);
}

#[aidoku_test]
fn search_urls() {
	let sort = |index| FilterValue::Sort {
		id: "sort".into(),
		index,
		ascending: false,
	};
	// popular defaults to the "Bugün" period shown in the filter
	assert_eq!(
		search_url(None, 1, &[sort(2)]),
		"https://okutoon.net/api/v1/series?sort=popular&period=daily&page=1"
	);
	assert_eq!(
		search_url(
			None,
			2,
			&[
				sort(2),
				FilterValue::Select {
					id: "period".into(),
					value: "monthly".into(),
				},
			]
		),
		"https://okutoon.net/api/v1/series?sort=popular&period=monthly&page=2"
	);
	// other sorts ignore the period; empty queries and "Tümü" are dropped
	assert_eq!(
		search_url(
			Some(""),
			1,
			&[FilterValue::Select {
				id: "genre".into(),
				value: "".into(),
			}]
		),
		"https://okutoon.net/api/v1/series?sort=newest&page=1"
	);
	assert_eq!(
		search_url(
			Some("blue lock"),
			1,
			&[FilterValue::Select {
				id: "genre".into(),
				value: "aksiyon".into(),
			}]
		),
		"https://okutoon.net/api/v1/series?sort=newest&q=blue%20lock&categories=aksiyon&page=1"
	);
}

#[aidoku_test]
fn last_catalog_page() {
	let list: SeriesList =
		parse(r#"{"data":[],"meta":{"page":96,"limit":24,"total":2300,"total_pages":96}}"#);
	let result: MangaPageResult = list.into();
	assert!(!result.has_next_page);
}

#[aidoku_test]
fn series_details() {
	let series: Response<Series> = parse(include_str!("../fixtures/series.json"));
	let manga: Manga = series.data.into();
	assert_eq!(manga.key, "the-remarried-empress");
	assert_eq!(manga.title, "The Remarried Empress");
	assert_eq!(
		manga.authors,
		Some(vec!["Alphatart".to_string(), "Sumpul".to_string()])
	);
	assert_eq!(manga.status, MangaStatus::Completed);
	assert_eq!(manga.viewer, Viewer::Webtoon);
	assert_eq!(manga.content_rating, ContentRating::Safe);
	assert!(
		manga
			.tags
			.as_deref()
			.is_some_and(|tags| tags.iter().any(|t| t == "Romantizm"))
	);
	assert_eq!(
		manga.url.as_deref(),
		Some("https://okutoon.net/the-remarried-empress")
	);
}

#[aidoku_test]
fn manga_type_and_rating() {
	let series: Series = parse(
		r#"{"slug":"x","title":"X","type":"manga","status":"hiatus","categories":[{"name":"Ecchi"}]}"#,
	);
	let manga: Manga = series.into();
	assert_eq!(manga.viewer, Viewer::RightToLeft);
	assert_eq!(manga.status, MangaStatus::Hiatus);
	assert_eq!(manga.content_rating, ContentRating::Suggestive);
}

#[aidoku_test]
fn chapter_list() {
	let list: ChapterList = parse(include_str!("../fixtures/chapters.json"));
	assert!(list.meta.has_more);
	assert_eq!(list.meta.offset, 50);
	let chapters: Vec<Chapter> = list.data.into_iter().map(Chapter::from).collect();
	assert_eq!(chapters.len(), 3);
	// decimal chapters come through as sort_number
	assert_eq!(chapters[0].key, "bolum-247-5");
	assert_eq!(chapters[0].chapter_number, Some(247.5));
	assert_eq!(chapters[1].chapter_number, Some(247.0));
	// "Bölüm N" titles only repeat the number
	assert!(chapters.iter().all(|c| c.title.is_none()));
	assert_eq!(
		chapters[1].url.as_deref(),
		Some("https://okutoon.net/the-remarried-empress/bolum-247")
	);
	// 2026-05-25 02:23:14 Istanbul time (UTC+3)
	assert_eq!(chapters[1].date_uploaded, Some(1_779_664_994));
}

#[aidoku_test]
fn chapter_real_title_is_kept() {
	let item: ChapterItem = parse(r#"{"slug":"bolum-1","display_number":"1","title":"Başlangıç"}"#);
	let chapter: Chapter = item.into();
	assert_eq!(chapter.title.as_deref(), Some("Başlangıç"));
	// without sort_number the number comes from display_number
	assert_eq!(chapter.chapter_number, Some(1.0));
}

#[aidoku_test]
fn reader_pages() {
	let reader: Response<ChapterPages> = parse(include_str!("../fixtures/chapter.json"));
	assert_eq!(reader.data.pages.len(), 3);
	assert!(
		reader.data.pages[0]
			.image_url
			.starts_with("https://cdn.okutoon.net/chapters/")
	);
}

#[aidoku_test]
fn deep_links() {
	let source = OkuToon;
	for url in [
		"https://okutoon.net/blue-lock",
		"https://okutoon.net/blue-lock/",
		"https://okutoon.net/blue-lock?ref=home",
		"https://okutoon.net/blue-lock/kutuphane/takip-edenler",
	] {
		assert!(
			matches!(
				source.handle_deep_link(url.to_string()),
				Ok(Some(DeepLinkResult::Manga { ref key })) if key == "blue-lock"
			),
			"{url}"
		);
	}
	assert!(matches!(
		source.handle_deep_link("https://okutoon.net/blue-lock/bolum-366#top".to_string()),
		Ok(Some(DeepLinkResult::Chapter { manga_key, key }))
			if manga_key == "blue-lock" && key == "bolum-366"
	));
	for url in [
		"https://okutoon.net/",
		"https://okutoon.net/tur/aksiyon",
		"https://okutoon.net/giris",
		"https://okutoon.net/arama?q=blue",
		"https://okutoon.net/sayfa/hakkimizda",
		"https://okutoon.net/yorumlar/176143",
		"https://okutoon.net/@kratos",
		"https://okutoon.net/forum/genel-sohbet",
		"https://okutoon.net.example.com/blue-lock",
	] {
		assert!(
			matches!(source.handle_deep_link(url.to_string()), Ok(None)),
			"{url}"
		);
	}
}
