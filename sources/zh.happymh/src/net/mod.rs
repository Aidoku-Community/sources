use crate::BASE_URL;
use aidoku::{
	alloc::{string::ToString as _, String},
	helpers::uri::encode_uri,
	imports::net::Request,
	prelude::*,
	FilterValue, Result,
};
use core::fmt::{Display, Formatter, Result as FmtResult};

const GENRE_OPTIONS: &[&str] = &[
	"热血", "格斗", "武侠", "魔幻", "魔法", "冒险", "爱情", "搞笑", "校园", "科幻", "后宫", "励志",
	"职场", "美食", "社会", "黑道", "战争", "历史", "悬疑", "竞技", "体育", "恐怖", "推理", "生活",
	"伪娘", "治愈", "神鬼", "四格", "百合", "耽美", "舞蹈", "侦探", "宅男", "音乐", "萌系", "古风",
	"恋爱", "都市", "性转", "穿越", "游戏", "其他", "爱妻", "日常", "腹黑", "古装", "仙侠", "生化",
	"修仙", "情感", "改编", "纯爱", "唯美", "蔷薇", "明星", "猎奇", "青春", "幻想", "惊奇", "彩虹",
	"奇闻", "权谋", "宅斗", "限制级", "装逼", "浪漫", "偶像", "大女主", "复仇", "虐心", "恶搞",
	"灵异", "惊险", "宠爱", "逆袭", "妖怪", "暧昧", "同人", "架空", "真人", "动作", "橘味", "宫斗",
	"脑洞", "漫改", "战斗", "丧尸", "美少女", "怪物", "系统", "智斗", "机甲", "高甜", "僵尸", "致郁",
	"电竞", "神魔", "异能", "末日", "乙女", "豪快", "奇幻", "绅士", "正能量", "宫廷", "亲情", "养成",
	"剧情", "轻小说", "暗黑", "长条", "玄幻", "霸总", "欧皇", "生存", "异世界", "其它", "C99", "节操",
	"AA", "影视化", "欧风", "女神", "爽感", "转生", "异形", "反套路", "双男主", "无敌流", "性转换",
	"重生", "血腥", "奇遇", "泛爱", "软萌", "邪恶", "资讯", "女频", "现言", "诡异"
];

const GENRE_IDS: &[&str] = &[
	"rexue",
	"gedou",
	"wuxia",
	"mohuan",
	"mofa",
	"maoxian",
	"aiqing",
	"gaoxiao",
	"xiaoyuan",
	"kehuan",
	"hougong",
	"lizhi",
	"zhichang",
	"meishi",
	"shehui",
	"heidao",
	"zhanzheng",
	"lishi",
	"xuanyi",
	"jingji",
	"tiyu",
	"kongbu",
	"tuili",
	"shenghuo",
	"weiniang",
	"zhiyu",
	"shengui",
	"sige",
	"baihe",
	"danmei",
	"wudao",
	"zhentan",
	"zhainan",
	"yinyue",
	"mengxi",
	"gufeng",
	"lianai",
	"dushi",
	"xingzhuan",
	"chuanyue",
	"youxi",
	"qita",
	"aiqi",
	"richang",
	"fuhei",
	"guzhuang",
	"xianxia",
	"shenghua",
	"xiuxian",
	"qinggan",
	"gaibian",
	"chunai",
	"weimei",
	"qiangwei",
	"mingxing",
	"lieqi",
	"qingchun",
	"huanxiang",
	"jingqi",
	"caihong",
	"qiwen",
	"quanmou",
	"zhaidou",
	"xianzhiji",
	"zhuangbi",
	"langman",
	"ouxiang",
	"danvzhu",
	"fuchou",
	"nuexin",
	"egao",
	"lingyi",
	"jingxian",
	"chongai",
	"nixi",
	"yaoguai",
	"aimei",
	"tongren",
	"jiakong",
	"zhenren",
	"dongzuo",
	"juwei",
	"gongdou",
	"naodong",
	"mangai",
	"zhandou",
	"sangshi",
	"meishaonv",
	"guaiwu",
	"xitong",
	"zhidou",
	"jijia",
	"gaotian",
	"jiangshi",
	"zhiyu",
	"dianjing",
	"shenmo",
	"yineng",
	"mori",
	"yinv",
	"haokuai",
	"qihuan",
	"shenshi",
	"zhengnengliang",
	"gongting",
	"qinqing",
	"yangcheng",
	"juqing",
	"qingxiaoshuo",
	"anhei",
	"changtiao",
	"xuanhuan",
	"bazong",
	"ouhuang",
	"shengcun",
	"yishijie",
	"qita",
	"C99",
	"jiecao",
	"AA",
	"yingshihua",
	"oufeng",
	"nvshen",
	"shuanggan",
	"zhuansheng",
	"yixing",
	"fantaolu",
	"shuangnanzhu",
	"wudiliu",
	"xingzhuanhuan",
	"zhongsheng",
	"xuexing",
	"qiyu",
	"fanai",
	"ruanmeng",
	"xiee",
	"zixun",
	"nvpin",
	"xianyan",
	"guiyi"
];

#[derive(Clone)]
pub enum Url {
	Filter {
		genre: String,
		area: String,
		audience: String,
		status: String,
		page: i32,
	},
	Search {
		query: String,
		page: i32,
	},
	Manga {
		id: String,
	},
}

impl Url {
	pub fn request(&self) -> Result<Request> {
		let url = self.to_string();
		let mut request = Request::get(url)?.header("Origin", BASE_URL);

		// Add special referer for search requests
		if let Url::Search { query, page } = self {
			request = Request::post(self.to_string())?
				.header("Origin", BASE_URL)
				.header("Referer", &format!("{}/sssearch", BASE_URL))
				.header("Content-Type", "application/x-www-form-urlencoded")
				.body(format!("searchkey={}&v=v2.13&page={}", query, page).as_bytes());
			return Ok(request);
		}

		// Add referer for filter requests
		if let Url::Filter { .. } = self {
			request = request.header("Referer", &format!("{}/latest", BASE_URL));
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
		let mut area = String::new();
		let mut audience = String::new();
		let mut status = String::from("-1");

		for filter in filters {
			match filter {
				FilterValue::Text { value, .. } => {
					return Ok(Self::Search {
						query: encode_uri(value.clone()),
						page,
					});
				}
				FilterValue::Select { id, value } => match id.as_str() {
					"地区" => area = value.clone(),
					"受众" => audience = value.clone(),
					"状态" => status = value.clone(),
					"类型" => genre = value.clone(),
					"genre" => {
						if let Some(index) = GENRE_OPTIONS
							.iter()
							.position(|&option| option == value.as_str())
						{
							if let Some(id) = GENRE_IDS.get(index) {
								genre = id.to_string();
							}
						}
					}
					_ => continue,
				}
				_ => continue,
			}
		}

		Ok(Self::Filter {
			genre,
			area,
			audience,
			status,
			page,
		})
	}

	pub fn manga(id: String) -> Self {
		Self::Manga { id }
	}
}

impl Display for Url {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		match self {
			Url::Filter {
				genre,
				area,
				audience,
				status,
				page,
			} => {
				write!(
					f, "{}/apis/c/index?&order=last_date&genre={}&area={}&audience={}&series_status={}&pn={}",
					BASE_URL, genre, area, audience, status, page
				)
			}
			Url::Search { query: _, page: _ } => {
				write!(f, "{}/v2.0/apis/manga/ssearch", BASE_URL)
			}
			Url::Manga { id } => {
				write!(f, "{}/manga/{}", BASE_URL, id)
			}
		}
	}
}
