use aidoku::{
	ContentRating, HomeComponent, HomeComponentValue, Link, Listing, ListingKind, Manga,
	MangaStatus, Result,
	alloc::{String, Vec, format},
	imports::net::Request,
};

use crate::graphql::{HOME_RECOMMENDATIONS_QUERY, HomeVariables};
use crate::models::{
	Cover, HomeManga, HomePopularData, HomeRecommendationsData, Localized, SearchManga, StaffMember,
};
use crate::{BASE_URL, Senkuro, USER_AGENT};

impl Senkuro {
	pub(crate) fn title(original: &str, titles: &[Localized]) -> String {
		for item in titles {
			if item.lang == "RU" {
				return item.content.clone();
			}
		}
		if !original.is_empty() {
			String::from(original)
		} else {
			titles
				.first()
				.map(|item| item.content.clone())
				.unwrap_or_default()
		}
	}
	pub(crate) fn rating(value: &str) -> ContentRating {
		match value {
			"EXPLICIT" => ContentRating::NSFW,
			"QUESTIONABLE" | "SENSITIVE" => ContentRating::Suggestive,
			"GENERAL" => ContentRating::Safe,
			_ => ContentRating::Unknown,
		}
	}

	pub(crate) fn status(value: &str) -> MangaStatus {
		match value {
			"ONGOING" => MangaStatus::Ongoing,
			"FINISHED" | "RELEASED" => MangaStatus::Completed,
			"SUSPENDED" => MangaStatus::Hiatus,
			"CANCELLED" => MangaStatus::Cancelled,
			_ => MangaStatus::Unknown,
		}
	}

	pub(crate) fn cover(cover: Option<Cover>) -> Option<String> {
		cover.and_then(|value| value.original.map(|image| image.url))
	}

	pub(crate) fn staff_names(staff: &[StaffMember], roles: &[&str]) -> Vec<String> {
		staff
			.iter()
			.filter(|member| {
				member
					.roles
					.iter()
					.any(|role| roles.iter().any(|needle| role.contains(needle)))
			})
			.map(|member| member.person.name.clone())
			.collect()
	}

	pub(crate) fn home_manga(item: HomeManga) -> Manga {
		let url = format!("{BASE_URL}/manga/{}", item.slug);
		Manga {
			key: item.slug,
			title: Self::title(&item.original_name.content, &item.titles),
			cover: Self::cover(item.cover),
			url: Some(url),
			status: Self::status(&item.status),
			content_rating: Self::rating(&item.rating),
			..Default::default()
		}
	}

	pub(crate) fn home_listing(id: &str, name: &str) -> Listing {
		Listing {
			id: id.into(),
			name: name.into(),
			kind: ListingKind::Default,
		}
	}

	pub(crate) fn home_component(title: &str, id: &str, entries: Vec<Manga>) -> HomeComponent {
		let links = entries.into_iter().map(Link::from).collect();
		HomeComponent {
			title: Some(title.into()),
			subtitle: None,
			value: HomeComponentValue::Scroller {
				entries: links,
				listing: Some(Self::home_listing(id, title)),
			},
		}
	}

	pub(crate) fn home_static_list(&self, query: &str) -> Result<Vec<Manga>> {
		let data: HomePopularData = self.graphql(query, HomeVariables { after: None })?;
		Ok(data
			.manga_popular_by_period
			.into_iter()
			.map(Self::home_manga)
			.collect())
	}

	pub(crate) fn home_recommendations(&self) -> Result<Vec<Manga>> {
		let data: HomeRecommendationsData =
			self.graphql(HOME_RECOMMENDATIONS_QUERY, HomeVariables { after: None })?;
		Ok(data
			.manga_recommendations
			.into_iter()
			.map(Self::home_manga)
			.collect())
	}

	pub(crate) fn search_manga(&self, item: SearchManga) -> Manga {
		let title = Self::title(&item.original_name, &item.titles);
		let url = format!("{BASE_URL}/manga/{}", item.slug);
		Manga {
			key: item.slug,
			title,
			cover: Self::cover(item.cover),
			url: Some(url),
			status: Self::status(&item.manga_status),
			content_rating: Self::rating(&item.manga_rating),
			..Default::default()
		}
	}

	pub(crate) fn fetch_description(&self, slug: &str) -> Option<String> {
		let url = format!("{BASE_URL}/manga/{slug}");
		let mut request = Request::get(url).ok()?;
		request = request.header("User-Agent", USER_AGENT);
		let response = request.send().ok()?;
		response
			.get_html()
			.ok()?
			.select_first("meta[name=description]")
			.and_then(|element| element.attr("content"))
	}
}
