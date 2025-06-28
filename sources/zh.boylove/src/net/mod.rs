use super::*;
use aidoku::{
	AidokuError,
	alloc::{format, string::ToString as _},
	helpers::uri::{QueryParameters, encode_uri_component},
	imports::{defaults::defaults_get, net::Request},
};
use core::{
	fmt::{Display, Formatter, Result as FmtResult},
	str::FromStr as _,
};
use strum::{AsRefStr, Display, EnumString, FromRepr};

#[derive(Display)]
#[strum(prefix = "https://boylove.cc")]
pub enum Url<'a> {
	#[strum(to_string = "/")]
	Home,
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
	#[strum(to_string = "/home/book/capter/id/{key}")]
	Chapter { key: &'a str },
	#[strum(to_string = "/home/Api/getDailyUpdate.html?{0}")]
	DailyUpdate(DailyUpdateQuery),
	#[strum(to_string = "/home/api/getpage/tp/1-{0}-{1}")]
	Listing(Listing, OffsetPage),
	#[strum(to_string = "/home/Api/getCnxh.html?{0}")]
	Random(RandomQuery),
	#[strum(to_string = "/home/index/dailyupdate1")]
	DailyUpdatePage,
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

	pub fn daily_update(id: &str, page: i32) -> Result<Self> {
		let day_of_week = DayOfWeek::from_str(id).map_err(|err| error!("{err:?}"))?;
		let query = DailyUpdateQuery::new(day_of_week, page);
		Ok(Self::DailyUpdate(query))
	}

	pub fn listing(id: &str, page: i32) -> Result<Self> {
		let listing = Listing::from_str(id).map_err(|err| error!("{err:?}"))?;
		let offset_page = OffsetPage::new(page);

		Ok(Self::Listing(listing, offset_page))
	}

	pub fn random() -> Self {
		let query = RandomQuery::new();
		Self::Random(query)
	}
}

impl<'a> Url<'a> {
	pub const fn manga(key: &'a str) -> Self {
		Self::Manga { key }
	}

	pub const fn chapter(key: &'a str) -> Self {
		Self::Chapter { key }
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
						"genre" => {
							let search_query = SearchQuery::new(value, page);
							return Ok(Self::Search(search_query));
						}
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

#[derive(Display, EnumString)]
pub enum Listing {
	#[strum(to_string = "recommend", serialize = "無碼專區")]
	Uncensored,
	#[strum(to_string = "topestmh", serialize = "排行榜")]
	Ranking,
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

pub struct DailyUpdateQuery(QueryParameters);

impl DailyUpdateQuery {
	fn new(day_of_week: DayOfWeek, page: i32) -> Self {
		let mut query = QueryParameters::new();
		query.push_encoded("widx", Some(day_of_week.as_ref()));
		query.push_encoded("limit", Some("18"));

		let offset_page = OffsetPage::new(page).to_string();
		query.push_encoded("page", Some(&offset_page));

		query.push_encoded("lastpage", Some("0"));

		Self(query)
	}
}

impl Display for DailyUpdateQuery {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		write!(f, "{}", self.0)
	}
}

pub struct OffsetPage(i32);

impl OffsetPage {
	fn new(page: i32) -> Self {
		let offset_page = page
			.checked_sub(1)
			.filter(|offset_page| *offset_page >= 0)
			.unwrap_or(0);
		Self(offset_page)
	}
}

impl Display for OffsetPage {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		write!(f, "{}", self.0)
	}
}

pub struct RandomQuery(QueryParameters);

impl RandomQuery {
	fn new() -> Self {
		let mut query = QueryParameters::new();
		query.push_encoded("limit", Some("5"));
		query.push_encoded("type", Some("1"));

		Self(query)
	}
}

impl Display for RandomQuery {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		write!(f, "{}", self.0)
	}
}

#[derive(AsRefStr, EnumString, Clone, Copy)]
enum DayOfWeek {
	#[strum(to_string = "11", serialize = "最新")]
	LastUpdated,
	#[strum(to_string = "0", serialize = "周一")]
	Monday,
	#[strum(to_string = "1", serialize = "周二")]
	Tuesday,
	#[strum(to_string = "2", serialize = "周三")]
	Wednesday,
	#[strum(to_string = "3", serialize = "周四")]
	Thursday,
	#[strum(to_string = "4", serialize = "周五")]
	Friday,
	#[strum(to_string = "5", serialize = "周六")]
	Saturday,
	#[strum(to_string = "6", serialize = "周日")]
	Sunday,
}

#[cfg(test)]
mod test;
