use aidoku::{
	Manga,
	alloc::{string::String, vec::Vec},
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct LoginStatus {
	pub access_token: String,
	pub refresh_token: String,
	pub is_subscribed: bool,
}

impl From<RefreshResponse> for LoginStatus {
	fn from(value: RefreshResponse) -> Self {
		Self {
			access_token: value.data.access_token,
			refresh_token: value.data.refresh_token,
			is_subscribed: value.data.subscription_status.has_subscription,
		}
	}
}

#[derive(Deserialize)]
pub struct RefreshResponse {
	pub data: RefreshResponseData,
}

#[derive(Deserialize)]
pub struct RefreshResponseData {
	pub access_token: String,
	pub refresh_token: String,
	pub subscription_status: RefreshSubscriptionStatus,
}

#[derive(Deserialize)]
pub struct RefreshSubscriptionStatus {
	pub has_subscription: bool,
}

#[derive(Deserialize)]
pub struct BookmarkResponse {
	pub data: Vec<BookmarkItem>,
	pub meta: BookmarkResponseMeta,
}

#[derive(Deserialize)]
pub struct BookmarkResponseMeta {
	pub total: i32,
}

#[derive(Deserialize)]
pub struct BookmarkItem {
	// pub id: i32,
	series: BookmarkSeries,
}

impl From<BookmarkItem> for Manga {
	fn from(value: BookmarkItem) -> Self {
		Manga {
			key: value.series.slug,
			title: value.series.title,
			cover: Some(value.series.cover_url),
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct BookmarkSeries {
	cover_url: String,
	slug: String,
	title: String,
}
