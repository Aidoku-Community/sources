use aidoku::{
	HomeComponent, HomeComponentValue, HomeLayout, HomePartialResult, Link, Listing, ListingKind,
	Manga, Result,
	alloc::{Vec, string::ToString, vec},
	imports::{net::Request, std::send_partial_result},
};

use crate::{
	auth::{self, AuthRequest},
	context::Context,
	endpoints::Url,
	models::responses::{MangaDetailResponse, MangaListResponse},
};

const POPULAR_TITLE: &str = "Популярное";
const POPULAR_SUBTITLE: &str = "За всё время";
const TRENDING_TITLE: &str = "Сейчас читают";
const TRENDING_SUBTITLE: &str = "Новинки дня";
const LATEST_TITLE: &str = "Последние обновления";

// Send initial layout structure
pub fn send_initial_layout(ctx: &Context) {
	let mut components = Vec::new();

	if ctx.site_id == 4 && !auth::is_authorized() {
		components.push(HomeComponent {
			title: Some("⚠️ ТРЕБУЕТСЯ АВТОРИЗАЦИЯ".to_string()),
			subtitle: Some(
				"Чтение глав невозможно без входа в аккаунт.\n\nКак войти:\n1. Нажмите 3 точки (справа сверху)\n2. Перейдите в Настройки ⚙️\n3. Нажмите «Войти через SocialLib»".to_string()
			),
			value: HomeComponentValue::empty_links(),
		});
	}

	components.push(create_home_component(
		POPULAR_TITLE,
		Some(POPULAR_SUBTITLE),
		HomeComponentValue::empty_big_scroller(),
	));
	components.push(create_home_component(
		TRENDING_TITLE,
		Some(TRENDING_SUBTITLE),
		HomeComponentValue::empty_scroller(),
	));
	components.push(create_home_component(
		LATEST_TITLE,
		None,
		HomeComponentValue::empty_scroller(),
	));

	send_partial_result(&HomePartialResult::Layout(HomeLayout { components }));
}

// Load popular manga with detailed information
pub fn load_popular_manga(ctx: &Context) -> Result<()> {
	let site_id_str = ctx.site_id.to_string();
	let params = vec![("site_id[]", site_id_str.as_str())];
	let url = Url::manga_search_with_params(&ctx.api_url, &params);

	let response = Request::get(&url)?
		.authed(ctx)?
		.get_json::<MangaListResponse>()?;

	let entries: Vec<Manga> = response
		.data
		.into_iter()
		.take(10)
		.map(|manga_data| {
			// Try to fetch details, fallback to basic if it fails
			fetch_manga_details(&manga_data.slug_url, ctx)
				.unwrap_or_else(|_| manga_data.into_manga(ctx))
		})
		.collect();

	send_popular_component(entries);
	Ok(())
}

// Load currently reading manga (daily trending)
pub fn load_currently_reading(ctx: &Context) -> Result<()> {
	let params = vec![("page", "1"), ("popularity", "1"), ("time", "day")];
	let url = Url::top_views_with_params(&ctx.api_url, &params);

	let response = Request::get(url)?
		.authed(ctx)?
		.get_json::<MangaListResponse>()?;

	let entries: Vec<Link> = response
		.data
		.into_iter()
		.take(30)
		.map(|manga_data| Link::from(manga_data.into_manga(ctx)))
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
pub fn load_latest_updates(ctx: &Context) -> Result<()> {
	let site_id_str = &ctx.site_id.to_string();
	let params = vec![
		("page", "1"),
		("site_id[]", site_id_str.as_str()),
		("sort_by", "last_chapter_at"),
	];
	let url = Url::manga_search_with_params(&ctx.api_url, &params);

	let response = Request::get(url)?
		.authed(ctx)?
		.get_json::<MangaListResponse>()?;

	let entries: Vec<Link> = response
		.data
		.into_iter()
		.take(30)
		.map(|manga_data| Link::from(manga_data.into_manga(ctx)))
		.collect();

	send_scroller_component(LATEST_TITLE, None, entries, "latest", LATEST_TITLE);
	Ok(())
}

// Helper functions
fn fetch_manga_details(slug_url: &str, ctx: &Context) -> Result<Manga> {
	let details_url = Url::manga_details_with_fields(
		&ctx.api_url,
		slug_url,
		&["summary", "tags", "authors", "artists"],
	);

	let manga = Request::get(details_url)?
		.authed(ctx)?
		.get_json::<MangaDetailResponse>()?
		.data
		.into_manga(ctx);

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
