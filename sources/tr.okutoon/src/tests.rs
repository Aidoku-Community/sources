use super::*;
use aidoku::alloc::vec;
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
	let manga = parse_details("blue-lock", &document);
	let chapters = parse_chapters("blue-lock", &document);
	assert_eq!(manga.title, "Blue Lock");
	assert_eq!(
		manga.authors.as_deref().unwrap_or(&vec![]),
		&["Muneyuki Kaneshiro".to_string()]
	);
	assert!(
		manga
			.description
			.as_deref()
			.unwrap_or_default()
			.contains("Blue Lock")
	);
	assert_eq!(manga.status, MangaStatus::Ongoing);
	assert_eq!(manga.viewer, Viewer::Webtoon);
	assert!(manga.tags.as_deref().unwrap_or(&vec![]).len() >= 3);
	assert_eq!(
		manga.cover.as_deref(),
		Some("https://om-cdn.okutoon.com/covers/cover_1784153823_245c17ff.webp")
	);

	assert_eq!(chapters.len(), 4);
	let newest = &chapters[0];
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

#[aidoku_test]
fn capitalised_months() {
	assert_eq!(
		parse_date("12 Şubat 2026"),
		Some(epoch_from_ymd(2026, 2, 12) + 43_200)
	);
	assert_eq!(
		parse_date("1 Ağustos 2026"),
		Some(epoch_from_ymd(2026, 8, 1) + 43_200)
	);
}

#[aidoku_test]
fn missing_json_ld_fields_fall_back() {
	// no image/description in JSON-LD: cover comes from og:image and the
	// description stays empty rather than Some("")
	let html = r#"<html><head><script type="application/ld+json">{"@graph":[{"@type":"ComicSeries","name":"Blue Lock"}]}</script>
		<meta property="og:image" content="https://om-cdn.okutoon.com/covers/fallback.webp"></head><body></body></html>"#;
	let document = Html::parse(html).expect("document");
	let manga = parse_details("blue-lock", &document);
	assert_eq!(manga.title, "Blue Lock");
	assert_eq!(
		manga.cover.as_deref(),
		Some("https://om-cdn.okutoon.com/covers/fallback.webp")
	);
	assert_eq!(manga.description, None);
}

#[aidoku_test]
fn deep_link_variants() {
	let source = OkuToon;
	for url in [
		"https://www.okutoon.com/blue-lock",
		"http://okutoon.com/blue-lock",
		"https://okutoon.com/blue-lock?ref=home",
		"https://okutoon.com/blue-lock/",
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
		source.handle_deep_link("https://okutoon.com/blue-lock/bolum-366#top".to_string()),
		Ok(Some(DeepLinkResult::Chapter { manga_key, key }))
			if manga_key == "blue-lock" && key == "bolum-366"
	));
	assert!(matches!(
		source.handle_deep_link("https://example.com/blue-lock".to_string()),
		Ok(None)
	));
}

#[aidoku_test]
fn pagination_from_next_link() {
	let card = r#"<a href="/blue-lock" class="series-card"><img src="https://om-cdn.okutoon.com/c.webp"><span class="series-card-title">Blue Lock</span></a>"#;
	let with_next =
		format!(r#"<main>{card}<a href="/tur?sayfa=2" rel="next">Sonraki »</a></main>"#);
	let last = format!("<main>{card}</main>");
	assert!(parse_cards(&Html::parse(with_next).expect("document")).has_next_page);
	assert!(!parse_cards(&Html::parse(last).expect("document")).has_next_page);
}

#[aidoku_test]
fn chapter_titles_dedup() {
	let html = r#"<div class="chapter-item" data-chapter-number="366.000" data-chapter-number-label="366">
		<a href="/blue-lock/bolum-366" class="chapter-item-link"></a>
		<span class="chapter-title">Bölüm 366</span></div>
	<div class="chapter-item" data-chapter-number="365.000" data-chapter-number-label="365">
		<a href="/blue-lock/bolum-365" class="chapter-item-link"></a>
		<span class="chapter-title">Final Maçı</span></div>"#;
	let chapters = parse_chapters("blue-lock", &Html::parse(html).expect("document"));
	assert_eq!(chapters[0].title, None);
	assert_eq!(chapters[1].title.as_deref(), Some("Final Maçı"));
}

#[aidoku_test]
fn turkish_folding_and_authors() {
	assert_eq!(fold_turkish("TAMAMLANDI"), "tamamlandi");
	assert_eq!(fold_turkish("Bırakıldı"), "birakildi");
	assert_eq!(fold_turkish("İPTAL"), "iptal");
	let html = r#"<html><head><script type="application/ld+json">{"@graph":[{"@type":"ComicSeries","name":"X","author":[{"name":"A"},{"name":"B"}]}]}</script></head>
		<body><p class="series-detail-kicker"><span>Manga</span> · <span>TAMAMLANDI</span></p></body></html>"#;
	let manga = parse_details("x", &Html::parse(html).expect("document"));
	assert_eq!(manga.authors, Some(vec!["A".to_string(), "B".to_string()]));
	assert_eq!(manga.status, MangaStatus::Completed);
	assert_eq!(manga.viewer, Viewer::RightToLeft);
}

#[aidoku_test]
fn image_urls() {
	assert_eq!(
		absolute_url("//om-cdn.okutoon.com/a.webp").as_deref(),
		Some("https://om-cdn.okutoon.com/a.webp")
	);
	assert_eq!(
		absolute_url("/img/a.webp").as_deref(),
		Some("https://okutoon.com/img/a.webp")
	);
	assert_eq!(absolute_url("data:image/gif;base64,AAA"), None);
	let html = r#"<img class="reader-page" src="data:image/gif;base64,AAA" data-src="https://om-cdn.okutoon.com/p1.webp">"#;
	let pages = parse_pages(&Html::parse(html).expect("document")).expect("pages");
	assert_eq!(pages.len(), 1);
}

#[aidoku_test]
fn deep_link_rejections() {
	let source = OkuToon;
	for url in [
		"https://okutoon.com.example.org/blue-lock",
		"https://okutoon.company/blue-lock",
		"https://okutoon.com/giris",
		"https://okutoon.com/arama?q=blue",
	] {
		assert!(
			matches!(source.handle_deep_link(url.to_string()), Ok(None)),
			"{url}"
		);
	}
	// non-bolum sub-pages open the series, not a bogus chapter
	assert!(matches!(
		source.handle_deep_link("https://okutoon.com/blue-lock/kutuphane/takip-edenler".to_string()),
		Ok(Some(DeepLinkResult::Manga { key })) if key == "blue-lock"
	));
	assert!(matches!(
		source.handle_deep_link("https://okutoon.com/register".to_string()),
		Ok(None)
	));
	assert!(matches!(
		source.handle_deep_link("https://okutoon.com/forum/genel-sohbet".to_string()),
		Ok(None)
	));
}
