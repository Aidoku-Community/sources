use crate::BASE_URL;
use aidoku::{
	Result,
	alloc::{String, Vec, string::ToString as _},
	helpers::uri::encode_uri,
	imports::net::Request,
};
use core::fmt::{Display, Formatter, Result as FmtResult};

#[derive(Clone)]
pub enum Url {
	/// `/i/sor.aspx?key={query}` — a single page, no pagination.
	Search { query: String },
	/// `/{category}/` for page 1, `/{category}/{page}.html` afterwards.
	List { category: String, page: i32 },
	/// `/{id}/` — book page with the full chapter list embedded in `#list`.
	Manga { id: String },
	/// `/{book_id}/{chapter_id}.html` (page 1) or
	/// `/{book_id}/{chapter_id}-{n}.html` for paginated chapters.
	Chapter {
		book_id: String,
		chapter_id: String,
		page: i32,
	},
}

impl Url {
	pub fn request(&self) -> Result<Request> {
		Ok(Request::get(self.to_string())?
			.header("Accept-Language", "zh-CN,zh;q=0.9")
			.header("Referer", BASE_URL))
	}

	/// Browsing mode: pick a category from the filters, defaulting to the
	/// completed-novel listing when no category is selected.
	pub fn from_filters(filters: Vec<aidoku::FilterValue>, page: i32) -> Self {
		let mut category = String::from(DEFAULT_LISTING);
		for filter in filters {
			if let aidoku::FilterValue::Select { id, value } = filter
				&& id.as_str() == "分類"
			{
				category = value;
			}
		}
		Self::List { category, page }
	}

	pub fn manga(id: &str) -> Self {
		Self::Manga { id: id.to_string() }
	}

	/// Chapter URL for a given body page; long chapters are split into
	/// `/{book}/{ch}-{n}.html` pages.
	pub fn chapter_page(book_id: &str, chapter_id: &str, page: i32) -> Self {
		Self::Chapter {
			book_id: book_id.to_string(),
			chapter_id: chapter_id.to_string(),
			page,
		}
	}
}

impl Display for Url {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		match self {
			Url::Search { query } => {
				write!(f, "{}/i/sor.aspx?key={}", BASE_URL, encode_uri(query))
			}
			Url::List { category, page } => {
				if *page <= 1 {
					write!(f, "{}/{}/", BASE_URL, category)
				} else {
					write!(f, "{}/{}/{}.html", BASE_URL, category, page)
				}
			}
			Url::Manga { id } => write!(f, "{}/{}/", BASE_URL, id),
			Url::Chapter {
				book_id,
				chapter_id,
				page,
			} => {
				if *page <= 1 {
					write!(f, "{}/{}/{}.html", BASE_URL, book_id, chapter_id)
				} else {
					write!(f, "{}/{}/{}-{}.html", BASE_URL, book_id, chapter_id, page)
				}
			}
		}
	}
}

pub(crate) const DEFAULT_LISTING: &str = "wanjie";
