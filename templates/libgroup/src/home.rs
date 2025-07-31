use aidoku::{
	HomeComponent, HomeComponentValue, HomeLayout, HomePartialResult, Link, Listing, ListingKind,
	Manga, Result,
	alloc::{Vec, string::ToString, vec},
	imports::{net::Request, std::send_partial_result},
};

use crate::{
	auth::AuthRequest,
	endpoints::Url,
	models::responses::{MangaDetailResponse, MangaListResponse},
};

const POPULAR_TITLE: &str = "Популярное";
const POPULAR_SUBTITLE: &str = "За всё время";
const TRENDING_TITLE: &str = "Сейчас читают";
const TRENDING_SUBTITLE: &str = "Новинки дня";
const LATEST_TITLE: &str = "Последние обновления";

// Send initial layout structure
pub fn send_initial_layout() {
	send_partial_result(&HomePartialResult::Layout(HomeLayout {
		components: vec![
			create_home_component(
				POPULAR_TITLE,
				Some(POPULAR_SUBTITLE),
				HomeComponentValue::empty_big_scroller(),
			),
			create_home_component(
				TRENDING_TITLE,
				Some(TRENDING_SUBTITLE),
				HomeComponentValue::empty_scroller(),
			),
			create_home_component(LATEST_TITLE, None, HomeComponentValue::empty_scroller()),
		],
	}));
}

// Load popular manga with detailed information
pub fn load_popular_manga(
	api_url: &str,
	site_id: &str,
	base_url: &str,
	cover_quality: &str,
) -> Result<()> {
	let params = vec![("site_id[]", site_id)];
	let url = Url::manga_search_with_params(api_url, &params);

	let response = Request::get(url)?
		.authed()?
		.get_json::<MangaListResponse>()?;

	let entries: Vec<Manga> = response
		.data
		.into_iter()
		.take(10)
		.map(|manga_data| {
			// Try to fetch details, fallback to basic if it fails
			fetch_manga_details(api_url, &manga_data.slug_url, base_url, cover_quality)
				.unwrap_or_else(|_| manga_data.into_manga(base_url, cover_quality))
		})
		.collect();

	send_popular_component(entries);
	Ok(())
}

// Load currently reading manga (daily trending)
pub fn load_currently_reading(
	api_url: &str,
	site_id: &str,
	base_url: &str,
	cover_quality: &str,
	user_agent: &str,
) -> Result<()> {
	let params = vec![("page", "1"), ("popularity", "1"), ("time", "day")];
	let url = Url::top_views_with_params(api_url, &params);

	let response = Request::get(url)?
		.header("Referer", api_url)
		.header("Site-Id", site_id)
		.header("User-Agent", user_agent)
		.authed()?
		.get_json::<MangaListResponse>()?;

	let entries: Vec<Link> = response
		.data
		.into_iter()
		.take(30)
		.map(|manga_data| Link::from(manga_data.into_manga(base_url, cover_quality)))
		.collect();

	send_scroller_component(
		TRENDING_TITLE,
		Some(TRENDING_SUBTITLE),
		entries,
		"currently_reading",
		TRENDING_TITLE,
	);
	Ok(())
}

// Load latest updates
pub fn load_latest_updates(
	api_url: &str,
	site_id: &str,
	base_url: &str,
	cover_quality: &str,
) -> Result<()> {
	let params = vec![
		("page", "1"),
		("site_id[]", site_id),
		("sort_by", "last_chapter_at"),
	];
	let url = Url::manga_search_with_params(api_url, &params);

	let response = Request::get(url)?
		.authed()?
		.get_json::<MangaListResponse>()?;

	let entries: Vec<Link> = response
		.data
		.into_iter()
		.take(30)
		.map(|manga_data| Link::from(manga_data.into_manga(base_url, cover_quality)))
		.collect();

	send_scroller_component(LATEST_TITLE, None, entries, "latest", LATEST_TITLE);
	Ok(())
}

// Helper functions
fn fetch_manga_details(
	api_url: &str,
	slug_url: &str,
	base_url: &str,
	cover_quality: &str,
) -> Result<Manga> {
	let details_url = Url::manga_details_with_fields(
		api_url,
		slug_url,
		&["summary", "tags", "authors", "artists"],
	);

	let manga = Request::get(details_url)?
		.authed()?
		.get_json::<MangaDetailResponse>()?
		.data
		.into_manga(base_url, cover_quality);

	Ok(manga)
}

fn create_home_component(
	title: &str,
	subtitle: Option<&str>,
	value: HomeComponentValue,
) -> HomeComponent {
	HomeComponent {
		title: Some(title.into()),
		subtitle: subtitle.map(|s| s.into()),
		value,
	}
}

fn send_popular_component(entries: Vec<Manga>) {
	send_partial_result(&HomePartialResult::Component(HomeComponent {
		title: Some(POPULAR_TITLE.into()),
		subtitle: Some(POPULAR_SUBTITLE.into()),
		value: HomeComponentValue::BigScroller {
			entries,
			auto_scroll_interval: Some(5.0),
		},
	}));
}

fn send_scroller_component(
	title: &str,
	subtitle: Option<&str>,
	entries: Vec<Link>,
	id: &str,
	name: &str,
) {
	send_partial_result(&HomePartialResult::Component(HomeComponent {
		title: Some(title.into()),
		subtitle: subtitle.map(|s| s.into()),
		value: HomeComponentValue::Scroller {
			entries,
			listing: Some(Listing {
				id: id.to_string(),
				name: name.to_string(),
				kind: ListingKind::Default,
			}),
		},
	}));
}
