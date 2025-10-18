use crate::BASE_URL;
use aidoku::{
	FilterValue, Result,
	alloc::{String, string::ToString as _},
	helpers::uri::encode_uri,
	imports::net::Request,
	prelude::*,
};
use core::fmt::{Display, Formatter, Result as FmtResult};

const GENRE_OPTIONS: &[&str] = &[
	"所有", "魔幻", "魔法", "熱血", "冒險", "懸疑", "偵探", "愛情", "戀愛", "校園", "搞笑", "四格",
	"科幻", "神鬼", "舞蹈", "音樂", "百合", "後宮", "機戰", "格鬥", "恐怖", "萌系", "武俠", "社會",
	"歷史", "耽美", "勵志", "職場", "生活", "治癒", "偽娘", "黑道", "戰爭", "競技", "體育", "美食",
	"腐女", "宅男", "推理", "雜誌",
];

const GENRE_IDS: &[&str] = &[
	"",
	"mohuan",
	"mofa",
	"rexue",
	"maoxian",
	"xuanyi",
	"zhentan",
	"aiqing",
	"lianai",
	"xiaoyuan",
	"gaoxiao",
	"sige",
	"kehuan",
	"shengui",
	"wudao",
	"yinyue",
	"baihe",
	"hougong",
	"jizhan",
	"gedou",
	"kongbu",
	"mengxi",
	"wuxia",
	"shehui",
	"lishi",
	"danmei",
	"lizhi",
	"zhichang",
	"shenghuo",
	"zhiyu",
	"weiniang",
	"heidao",
	"zhanzheng",
	"jingji",
	"tiyu",
	"meishi",
	"funv",
	"zhainan",
	"tuili",
	"zazhi",
];

#[derive(Clone)]
pub enum Url {
	Filter {
		tag: String,
		country: String,
		audience: String,
		year: String,
		end: String,
		sort: String,
		page: i32,
	},
	Search {
		query: String,
		page: i32,
	},
	Author {
		author: String,
	},
	Manga {
		id: String,
	},
	ChapterList {
		id: String,
	},
	Chapter {
		chapter_id: String,
	},
}

impl Url {
	pub fn request(&self) -> Result<Request> {
		let url = self.to_string();
		let mut request = Request::get(url)?.header("Origin", BASE_URL);

		// Add special referer for search requests
		if let Url::Search { .. } = self {
			request = request.header("Referer", &format!("{}/comics", BASE_URL));
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

		let mut tag = String::new();
		let mut country = String::new();
		let mut audience = String::new();
		let mut year = String::new();
		let mut end = String::new();
		let mut sort = String::new();

		for filter in filters {
			match filter {
				FilterValue::Text { id, value } => match id.as_str() {
					"author" => {
						// Author search: /comics?filter[author]=encoded_value
						return Ok(Self::Author {
							author: encode_uri(value.clone()),
						});
					}
					_ => {
						// Title search
						return Ok(Self::Search {
							query: encode_uri(value.clone()),
							page,
						});
					}
				},
				FilterValue::Select { id, value } => match id.as_str() {
					"地区" => country = value.clone(),
					"受众" => audience = value.clone(),
					"年份" => year = value.clone(),
					"进度" => end = value.clone(),
					"类型" => tag = value.clone(),
					"genre" => {
						if let Some(index) = GENRE_OPTIONS
							.iter()
							.position(|&option| option == value.as_str())
							&& let Some(id) = GENRE_IDS.get(index)
						{
							tag = id.to_string();
						}
					}
					_ => {}
				},
				FilterValue::Sort {
					id,
					index,
					ascending,
				} => {
					if id.as_str() == "排序" {
						let sorts = ["", "update", "views"];
						if let Some(s) = sorts.get(*index as usize) {
							sort = s.to_string();
							if !sort.is_empty() && !*ascending {
								sort = format!("-{}", sort);
							}
						}
					}
				}
				_ => {}
			}
		}

		Ok(Self::Filter {
			tag,
			country,
			audience,
			year,
			end,
			sort,
			page,
		})
	}

	pub fn manga(id: String) -> Self {
		Self::Manga { id }
	}

	pub fn chapter_list(id: String) -> Self {
		Self::ChapterList { id }
	}

	pub fn chapter(chapter_id: String) -> Self {
		Self::Chapter { chapter_id }
	}
}

impl Display for Url {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		match self {
			Url::Filter {
				tag,
				country,
				audience,
				year,
				end,
				sort,
				page,
			} => {
				write!(
					f,
					"{}/comics?filter[tag]={}&filter[country]={}&filter[audience]={}&filter[year]={}&filter[end]={}&sort={}&page={}",
					BASE_URL, tag, country, audience, year, end, sort, page
				)
			}
			Url::Search { query, page } => {
				write!(f, "{}/comics?q={}&page={}", BASE_URL, query, page)
			}
			Url::Author { author } => {
				write!(f, "{}/comics?filter[author]={}", BASE_URL, author)
			}
			Url::Manga { id } => {
				write!(f, "{}/comics/{}", BASE_URL, id)
			}
			Url::ChapterList { id } => {
				write!(f, "{}/comics/{}", BASE_URL, id)
			}
			Url::Chapter { chapter_id } => {
				write!(f, "{}/chapters/{}", BASE_URL, chapter_id)
			}
		}
	}
}
