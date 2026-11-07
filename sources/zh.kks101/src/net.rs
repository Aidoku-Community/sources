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
	Search {
		query: String,
		page: i32,
	},
	/// `/novels/{order}_{tagid}_{isfull}_{page}.html`
	List {
		order: &'static str,
		tagid: String,
		isfull: String,
		page: i32,
	},
	Manga {
		id: String,
	},
	/// Full chapter list (`/ajax_novels/chapterlist/{id}.html`). Served
	/// behind a Cloudflare challenge; the source falls back to `Catalog`
	/// when the response is a challenge page.
	CatalogAjax {
		id: String,
	},
	/// The book page's embedded catalog (`/book/{id}/index.html`), which
	/// only lists the first and last chapters.
	Catalog {
		id: String,
	},
	Chapter {
		book_id: String,
		chapter_id: String,
	},
}

impl Url {
	pub fn request(&self) -> Result<Request> {
		Ok(Request::get(self.to_string())?
			.header("Accept-Language", "zh-CN,zh;q=0.9")
			.header("Referer", BASE_URL))
	}

	pub fn from_query_or_filters(
		query: Option<&str>,
		page: i32,
		filters: Vec<aidoku::FilterValue>,
	) -> Result<Self> {
		if let Some(q) = query.map(str::trim).filter(|q| !q.is_empty()) {
			return Ok(Self::Search {
				query: encode_uri(q),
				page,
			});
		}

		let mut order: &'static str = "newhot";
		let mut tagid = String::from("0");
		let mut isfull = String::from("0");

		for filter in filters {
			match filter {
				aidoku::FilterValue::Select { id, value } => match id.as_str() {
					"分類" => tagid = value,
					"連載狀態" => isfull = value,
					_ => {}
				},
				aidoku::FilterValue::Sort { index, .. } => {
					let orders = ["newhot", "weekvisit", "allvote"];
					if let Some(o) = orders.get(index as usize) {
						order = o;
					}
				}
				_ => {}
			}
		}

		Ok(Self::List {
			order,
			tagid,
			isfull,
			page,
		})
	}

	pub fn manga(id: &str) -> Self {
		Self::Manga { id: id.to_string() }
	}

	pub fn catalog_ajax(id: &str) -> Self {
		Self::CatalogAjax { id: id.to_string() }
	}

	pub fn catalog(id: &str) -> Self {
		Self::Catalog { id: id.to_string() }
	}

	pub fn chapter(book_id: &str, chapter_id: &str) -> Self {
		Self::Chapter {
			book_id: book_id.to_string(),
			chapter_id: chapter_id.to_string(),
		}
	}
}

impl Display for Url {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		match self {
			Url::Search { query, page } => {
				write!(f, "{}/search/{}/{}.html", BASE_URL, query, page)
			}
			Url::List {
				order,
				tagid,
				isfull,
				page,
			} => write!(
				f,
				"{}/novels/{}_{}_{}_{}.html",
				BASE_URL, order, tagid, isfull, page
			),
			Url::Manga { id } => write!(f, "{}/book/{}.html", BASE_URL, id),
			Url::CatalogAjax { id } => {
				write!(f, "{}/ajax_novels/chapterlist/{}.html", BASE_URL, id)
			}
			Url::Catalog { id } => write!(f, "{}/book/{}/index.html", BASE_URL, id),
			Url::Chapter {
				book_id,
				chapter_id,
			} => write!(f, "{}/txt/{}/{}.html", BASE_URL, book_id, chapter_id),
		}
	}
}
