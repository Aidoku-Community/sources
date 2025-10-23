use crate::BASE_URL;
use aidoku::{
	alloc::{string::ToString as _, String, Vec},
	helpers::uri::encode_uri,
	imports::net::Request,
	prelude::*,
	FilterValue, Result,
};
use core::fmt::{Display, Formatter, Result as FmtResult};

const GENRE: &[&str] = &["jp", "cn", "kr", "ou-mei", "qita", "hots"];

const TAG_OPTIONS: &[&str] = &[
	"复仇", "古风", "奇幻", "逆袭", "恋爱", "异能", "宅向", "穿越", "热血", "纯爱", "系统", "重生",
	"冒险", "灵异", "女主", "剧情", "恋爱", "玄幻", "女神", "科幻", "魔幻", "推理", "励志", "治愈",
	"都市", "异性", "青春", "末日", "悬疑", "修仙", "战斗",
];

const TAG_IDS: &[&str] = &[
	"fuchou",
	"gufeng",
	"qihuan",
	"nixi",
	"lianai",
	"yineng",
	"zhaixiang",
	"chuanyue",
	"rexue",
	"chunai",
	"xitong",
	"chongsheng",
	"maoxian",
	"lingyi",
	"danvzhu",
	"juqing",
	"lianai",
	"xuanhuan",
	"nvshen",
	"kehuan",
	"mohuan",
	"tuili",
	"lieqi",
	"zhiyu",
	"dushi",
	"yixing",
	"qingchun",
	"mori",
	"xuanyi",
	"xiuxian",
	"zhandou",
];

#[derive(Clone)]
pub enum Url {
	Filter { genre: String, page: i32 },
	Search { query: String, page: i32 },
	Manga { id: String },
}

impl Url {
	pub fn request(&self) -> Result<Request> {
		let url = self.to_string();
		let mut request = Request::get(url)?.header("Origin", BASE_URL);

		// Add special referer for search requests
		if let Url::Search { .. } = self {
			request = request.header("Referer", &format!("{}/s/", BASE_URL));
		}

		Ok(request)
	}

	pub fn from_query_or_filters(
		query: Option<&str>,
		page: i32,
		filters: &[FilterValue],
	) -> Result<Self> {
		if let Some(q) = query {
			return Ok(Self::Search {
				query: encode_uri(q),
				page,
			});
		}

		let mut genre = String::new();

		for filter in filters {
			match filter {
				FilterValue::Text { value, .. } => {
					// Title search
					return Ok(Self::Search {
						query: encode_uri(value.clone()),
						page,
					});
				}
				FilterValue::Select { id, value } => match id.as_str() {
					"类型" => genre = value.clone(),
					"genre" => {
						if let Some(index) = TAG_OPTIONS
							.iter()
							.position(|&option| option == value.as_str())
						{
							if let Some(id) = TAG_IDS.get(index) {
								genre = id.to_string();
							}
						}
					}
					_ => {}
				},
				_ => {}
			}
		}

		Ok(Self::Filter { genre, page })
	}

	pub fn manga(id: String) -> Self {
		Self::Manga { id }
	}
}

impl Display for Url {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		match self {
			Url::Filter { genre, page } => {
				let genre_str = if genre.is_empty() {
					String::from("manga")
				} else if GENRE.contains(&genre.as_str()) {
					format!("manga-genre/{}", genre)
				} else {
					format!("manga-tag/{}", genre)
				};
				write!(f, "{}/{}/page/{}", BASE_URL, genre_str, page)
			}
			Url::Search { query, page } => {
				write!(f, "{}/s/{}?page={}", BASE_URL, query, page)
			}
			Url::Manga { id } => {
				let ids = id.split("/").collect::<Vec<&str>>();
				write!(f, "{}/manga/{}", BASE_URL, ids[0])
			}
		}
	}
}
