use super::*;
use aidoku::{
	AidokuError,
	alloc::{format, string::ToString as _},
	error,
	helpers::uri::{QueryParameters, encode_uri_component},
	imports::{defaults::defaults_get, net::Request},
};
use core::{
	fmt::{Display, Formatter, Result as FmtResult},
	str::FromStr as _,
};
use strum::{Display, EnumString, FromRepr};

#[derive(Display)]
#[strum(prefix = "https://boylove.cc")]
pub enum Url<'a> {
	#[strum(to_string = "{0}")]
	Abs(&'a str),
	#[strum(to_string = "/home/user/to{0}.html")]
	ChangeCharset(Charset),
	#[strum(to_string = "/home/book/cate.html")]
	FiltersPage,
	#[strum(
		to_string = "/home/api/cate/tp/1-{tags}-{status}-{sort_by}-{page}-{content_rating}-1-{view_permission}"
	)]
	Filters {
		tags: Tags<'a>,
		status: Status,
		sort_by: Sort,
		page: i32,
		content_rating: ContentRating,
		view_permission: ViewPermission,
	},
	#[strum(to_string = "/home/api/searchk?{0}")]
	Search(SearchQuery),
	#[strum(to_string = "/home/book/index/id/{key}")]
	Manga { key: &'a str },
}

impl Url<'_> {
	pub fn request(&self) -> Result<Request> {
		let request = Request::get(self.to_string())?.header(
			"User-Agent",
			"Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
			 AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Safari/605.1.15",
		);
		Ok(request)
	}
}

impl<'a> Url<'a> {
	pub const fn manga(key: &'a str) -> Self {
		Self::Manga { key }
	}

	pub fn from_query_or_filters(
		query: Option<&str>,
		page: i32,
		filters: &'a [FilterValue],
	) -> Result<Self> {
		if let Some(keyword) = query {
			let search_query = SearchQuery::new(keyword, page);
			return Ok(Self::Search(search_query));
		}

		macro_rules! init {
			($($filter:ident: $Filter:ident),+) => {
				$(let mut $filter = $Filter::default();)+
			};
		}
		init!(
			tags: Tags,
			status: Status,
			sort_by: Sort,
			content_rating: ContentRating,
			view_permission: ViewPermission
		);

		for filter in filters {
			#[expect(clippy::match_wildcard_for_single_variants)]
			match filter {
				FilterValue::Text { id, value } => match id.as_str() {
					"author" => {
						let search_query = SearchQuery::new(value, page);
						return Ok(Self::Search(search_query));
					}
					_ => bail!("Invalid text filter ID: `{id}`"),
				},

				FilterValue::Sort { id, index, .. } => match id.as_str() {
					"排序方式" => {
						let discriminant = (*index).try_into().map_err(AidokuError::message)?;
						sort_by = Sort::from_repr(discriminant).unwrap_or_default();
					}
					_ => bail!("Invalid sort filter ID: `{id}`"),
				},

				FilterValue::Select { id, value } => {
					macro_rules! get_filter {
						($Filter:ident) => {
							$Filter::from_str(value).map_err(|err| error!("{err:?}"))?
						};
					}
					match id.as_str() {
						"閱覽權限" => view_permission = get_filter!(ViewPermission),
						"連載狀態" => status = get_filter!(Status),
						"內容分級" => content_rating = get_filter!(ContentRating),
						_ => bail!("Invalid select filter ID: `{id}`"),
					}
				}

				FilterValue::MultiSelect { id, included, .. } => match id.as_str() {
					"標籤" => tags.0 = included,
					_ => bail!("Invalid multi-select filter ID: `{id}`"),
				},

				_ => bail!("Invalid filter: `{filter:?}`"),
			}
		}

		Ok(Self::Filters {
			tags,
			status,
			sort_by,
			page,
			content_rating,
			view_permission,
		})
	}
}

impl From<Url<'_>> for String {
	fn from(url: Url<'_>) -> Self {
		url.to_string()
	}
}

#[derive(Display)]
pub enum Charset {
	#[strum(to_string = "S")]
	Simplified,
	#[strum(to_string = "T")]
	Traditional,
}

impl Charset {
	pub fn from_settings() -> Result<Self> {
		let is_traditional_chinese = defaults_get("isTraditionalChinese")
			.ok_or_else(|| error!("Default does not exist for key: `isTraditionalChinese`"))?;
		let charset = if is_traditional_chinese {
			Self::Traditional
		} else {
			Self::Simplified
		};
		Ok(charset)
	}
}

#[derive(Display, Default, EnumString)]
pub enum Status {
	#[default]
	#[strum(to_string = "2", serialize = "全部")]
	All,
	#[strum(to_string = "0", serialize = "連載中")]
	Ongoing,
	#[strum(to_string = "1", serialize = "已完結")]
	Completed,
}

#[derive(Display, Default, FromRepr)]
pub enum Sort {
	#[strum(to_string = "0")]
	Popularity,
	#[default]
	#[strum(to_string = "1")]
	LastUpdated,
}

#[derive(Display, Default, EnumString)]
pub enum ContentRating {
	#[default]
	#[strum(to_string = "0", serialize = "全部")]
	All,
	#[strum(to_string = "1", serialize = "清水")]
	Safe,
	#[strum(to_string = "2", serialize = "有肉")]
	Nsfw,
}

#[derive(Display, Default, EnumString)]
pub enum ViewPermission {
	#[default]
	#[strum(to_string = "2", serialize = "全部")]
	All,
	#[strum(to_string = "0", serialize = "一般")]
	Basic,
	#[strum(to_string = "1", serialize = "VIP")]
	Vip,
}

#[derive(Default)]
pub struct Tags<'a>(&'a [String]);

impl Display for Tags<'_> {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		let tags = if self.0.is_empty() {
			"0".into()
		} else {
			self.0
				.iter()
				.map(encode_uri_component)
				.collect::<Vec<_>>()
				.join("+")
		};
		write!(f, "{tags}")
	}
}

pub struct SearchQuery(QueryParameters);

impl SearchQuery {
	fn new(keyword: &str, page: i32) -> Self {
		let mut query = QueryParameters::new();
		query.push("keyword", Some(keyword));
		query.push_encoded("type", Some("1"));
		query.push_encoded("pageNo", Some(&page.to_string()));

		Self(query)
	}
}

impl Display for SearchQuery {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		write!(f, "{}", self.0)
	}
}

#[cfg(test)]
mod test;
