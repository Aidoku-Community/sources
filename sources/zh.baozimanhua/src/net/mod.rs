use crate::BASE_URL;
use aidoku::{
	FilterValue, Result,
	alloc::{String, string::ToString as _},
	helpers::uri::encode_uri,
	imports::net::Request,
};
use core::fmt::{Display, Formatter, Result as FmtResult};

const GENRE_OPTIONS: &[&str] = &[
	"戀愛",
	"純愛",
	"古風",
	"異能",
	"懸疑",
	"劇情",
	"科幻",
	"奇幻",
	"玄幻",
	"穿越",
	"冒險",
	"推理",
	"武俠",
	"格鬥",
	"戰爭",
	"熱血",
	"搞笑",
	"大女主",
	"都市",
	"總裁",
	"後宮",
	"日常",
	"韓漫",
	"少年",
	"其他",
];

const GENRE_IDS: &[&str] = &[
	"lianai",
	"chunai",
	"gufeng",
	"yineng",
	"xuanyi",
	"juqing",
	"kehuan",
	"qihuan",
	"xuanhuan",
	"chuanyue",
	"mouxian",
	"tuili",
	"wuxia",
	"gedou",
	"zhanzheng",
	"rexie",
	"gaoxiao",
	"danuzhu",
	"dushi",
	"zongcai",
	"hougong",
	"richang",
	"hanman",
	"shaonian",
	"qita",
];

#[derive(Clone)]
pub enum Url {
	Filter {
		genre: String,
		region: String,
		status: String,
		filter: String,
		page: i32,
	},
	Search {
		query: String,
	},
	Manga {
		id: String,
	},
	Chapter {
		manga_id: String,
		chapter_id: String,
	},
}

impl Url {
	pub fn request(&self) -> Result<Request> {
		let url = self.to_string();
		Ok(Request::get(url)?
			.header("Origin", BASE_URL)
			.header("Accept-Language", "zh-CN,zh;q=0.9")
			.header("Referer", BASE_URL))
	}

	pub fn from_query_or_filters(
		query: Option<&str>,
		page: i32,
		filters: &[FilterValue],
	) -> Result<Self> {
		if let Some(q) = query {
			return Ok(Self::Search {
				query: encode_uri(q),
			});
		}

		let mut genre = "all".to_string();
		let mut region = "all".to_string();
		let mut status = "all".to_string();
		let mut filter = "*".to_string();

		for filter_value in filters {
			match filter_value {
				FilterValue::Text { value, .. } => {
					return Ok(Self::Search {
						query: encode_uri(value.clone()),
					});
				}
				FilterValue::Select { id, value } => match id.as_str() {
					"地區" => region = value.clone(),
					"狀態" => status = value.clone(),
					"字母" => filter = value.clone(),
					"類型" => genre = value.clone(),
					"genre" => {
						if let Some(index) = GENRE_OPTIONS
							.iter()
							.position(|&option| option == value.as_str())
							&& let Some(id) = GENRE_IDS.get(index)
						{
							genre = id.to_string();
						}
					}
					_ => {}
				},
				_ => {}
			}
		}

		Ok(Self::Filter {
			genre,
			region,
			status,
			filter,
			page,
		})
	}

	pub fn manga(id: String) -> Self {
		Self::Manga { id }
	}

	pub fn chapter(manga_id: String, chapter_id: String) -> Self {
		Self::Chapter {
			manga_id,
			chapter_id,
		}
	}
}

impl Display for Url {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		match self {
			Url::Filter {
				genre,
				region,
				status,
				filter,
				page,
			} => {
				write!(
					f,
					"{}/api/bzmhq/amp_comic_list?type={}&region={}&state={}&filter={}&page={}",
					BASE_URL, genre, region, status, filter, page
				)
			}
			Url::Search { query } => {
				write!(f, "{}/search?q={}", BASE_URL, query)
			}
			Url::Manga { id } => {
				write!(f, "{}/comic/{}", BASE_URL, id)
			}
			Url::Chapter {
				manga_id,
				chapter_id,
			} => {
				write!(
					f,
					"{}/user/page_direct?comic_id={}&section_slot=0&chapter_slot={}",
					BASE_URL, manga_id, chapter_id
				)
			}
		}
	}
}
