use aidoku::{Listing, ListingKind, alloc::string::String};
use serde::Deserialize;

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct InkMangaAlias {
	pub id: String,
	pub slug: String,
	#[serde(rename = "bookId")]
	pub book_id: String,
	#[serde(rename = "serviceName")]
	pub service_name: String,
	#[serde(rename = "createdAt")]
	pub created_at: String,
	#[serde(rename = "updatedAt")]
	pub updated_at: String,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct InkLabel {
	pub id: String,
	pub slug: String,
	pub name: String,
	pub featured: bool,
	#[serde(rename = "hideForUnauthorized")]
	pub hide_for_unauthorized: bool,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct InkRelation {
	#[serde(rename = "type")]
	pub type_relation: String,
	pub publisher: Option<InkRelationPublisher>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct InkRelationPublisher {
	pub id: String,
	pub slug: String,
	pub name: String,
	pub kind: String,
}

impl InkLabel {
	pub fn into_listing(self, kind: ListingKind) -> Listing {
		Listing {
			id: self.id,
			name: self.name,
			kind,
		}
	}
}

