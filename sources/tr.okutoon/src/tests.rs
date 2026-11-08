use super::*;
use aidoku::imports::html::Html;
use aidoku_test::aidoku_test;

#[aidoku_test]
fn catalog_cards() {
	let document = Html::parse(include_str!("../fixtures/search.html")).expect("search document");
	let result = parse_cards(&document);
	assert!(!result.entries.is_empty());
	let blue_lock = result
		.entries
		.iter()
		.find(|manga| manga.key == "blue-lock")
		.expect("blue lock card");
	assert_eq!(blue_lock.title, "Blue Lock");
	assert_eq!(
		blue_lock.cover.as_deref(),
		Some("https://om-cdn.okutoon.com/covers/cover_1784153823_245c17ff.webp")
	);
	// every entry has a non-empty key and title
	assert!(
		result
			.entries
			.iter()
			.all(|m| !m.key.is_empty() && !m.title.is_empty())
	);
}

#[aidoku_test]
fn series_details_and_chapters() {
	let document = Html::parse(include_str!("../fixtures/series.html")).expect("series document");
	let series = parse_series("blue-lock", &document);
	assert_eq!(series.manga.title, "Blue Lock");
	assert_eq!(
		series.manga.authors.as_deref().unwrap_or(&vec![]),
		&["Muneyuki Kaneshiro".to_string()]
	);
	assert!(
		series
			.manga
			.description
			.as_deref()
			.unwrap_or_default()
			.contains("Blue Lock")
	);
	assert_eq!(series.manga.status, MangaStatus::Ongoing);
	assert_eq!(series.manga.viewer, Viewer::Webtoon);
	assert!(series.manga.tags.as_deref().unwrap_or(&vec![]).len() >= 3);
	assert_eq!(
		series.manga.cover.as_deref(),
		Some("https://om-cdn.okutoon.com/covers/cover_1784153823_245c17ff.webp")
	);

	assert_eq!(series.chapters.len(), 4);
	let newest = &series.chapters[0];
	assert_eq!(newest.key, "bolum-366");
	assert_eq!(newest.chapter_number, Some(366.0));
	assert_eq!(
		newest.url.as_deref(),
		Some("https://okutoon.com/blue-lock/bolum-366")
	);
	// "3 Kasım 2026" at noon UTC
	assert_eq!(
		newest.date_uploaded,
		Some(epoch_from_ymd(2026, 11, 3) + 43_200)
	);
}

#[aidoku_test]
fn chapter_sub_numbers() {
	// the site encodes re-uploads as extra chapters like "bolum-364-2" with
	// data-chapter-number 364.200
	let html = r#"<div class="chapter-item" data-chapter-number="364.200">
		<a href="/blue-lock/bolum-364-2" class="chapter-item-link"></a>
		<span class="chapter-main"><span class="chapter-title">Bölüm 364.2</span>
		<span class="chapter-date">6 Kasım 2026</span></span>
	</div>"#;
	let document = Html::parse(html).expect("document");
	let chapters = parse_chapters("blue-lock", &document);
	assert_eq!(chapters.len(), 1);
	assert_eq!(chapters[0].key, "bolum-364-2");
	assert_eq!(chapters[0].chapter_number, Some(364.2));
}

#[aidoku_test]
fn reader_pages() {
	let document = Html::parse(include_str!("../fixtures/reader.html")).expect("reader document");
	let pages = parse_pages(&document).expect("pages");
	assert_eq!(pages.len(), 5);
	let PageContent::Url(url, _) = &pages[0].content else {
		panic!("expected page url");
	};
	assert_eq!(
		url,
		"https://om-cdn.okutoon.com/chapters/538/64813/blue-lock_bolum-1_001.webp"
	);
}

#[aidoku_test]
fn dates_and_slugs() {
	assert_eq!(
		parse_date("3 Kasım 2026"),
		Some(epoch_from_ymd(2026, 11, 3) + 43_200)
	);
	assert_eq!(parse_date("yesterday"), None);
	assert_eq!(&slug_from_url("/blue-lock/bolum-366"), "bolum-366");
	assert_eq!(&slug_from_url("blue-lock"), "blue-lock");
}

#[aidoku_test]
fn deep_links() {
	let source = OkuToon;
	assert!(matches!(
		source.handle_deep_link("https://okutoon.com/blue-lock".to_string()),
		Ok(Some(DeepLinkResult::Manga { key })) if key == "blue-lock"
	));
	assert!(matches!(
		source
			.handle_deep_link("https://okutoon.com/blue-lock/bolum-366".to_string()),
		Ok(Some(DeepLinkResult::Chapter { manga_key, key }))
			if manga_key == "blue-lock" && key == "bolum-366"
	));
	assert!(matches!(
		source.handle_deep_link("https://okutoon.com/tur/aksiyon".to_string()),
		Ok(None)
	));
	assert!(matches!(
		source.handle_deep_link("https://okutoon.com/".to_string()),
		Ok(None)
	));
}
