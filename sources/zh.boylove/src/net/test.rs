#![expect(clippy::unwrap_used)]

use super::*;
use aidoku_test::aidoku_test;
use core::fmt::Debug;
use paste::paste;

macro_rules! change_charset_to {
	($Charset:ident, $expected_url:literal, $expected_lang:literal) => {
		paste! {
			#[aidoku_test]
			fn [<change_charset_to_ $Charset:lower>]() {
				let url = Url::ChangeCharset(Charset::$Charset);
				assert_eq!(url, $expected_url);
				assert!(url.request().unwrap().send().unwrap().get_header("Set-Cookie").unwrap().contains(&format!("lang={}", $expected_lang)));
			}
		}
	};
}
change_charset_to!(Traditional, "https://boylove.cc/home/user/toT.html", "TW");
change_charset_to!(Simplified, "https://boylove.cc/home/user/toS.html", "CN");

#[aidoku_test]
fn filters_page() {
	let url = Url::FiltersPage;
	let expected_url = "https://boylove.cc/home/book/cate.html";
	assert_eq!(url, expected_url);
	assert_eq!(
		url.request()
			.unwrap()
			.html()
			.unwrap()
			.select_first("ul.stui-header__menu > li.active > a")
			.unwrap()
			.attr("abs:href")
			.unwrap(),
		expected_url
	);
}

macro_rules! from_filters {
	($name:ident, ($page:literal$(, $filter:expr)*), $expected_url:literal, $code:literal) => {
		paste! {
			#[aidoku_test]
			fn [<from_filters_ $name>]() {
				let filters = [$($filter,)*];
				let url = Url::from_query_or_filters(None, $page, &filters).unwrap();
				assert_eq!(url, $expected_url);
				assert!(url.request().unwrap().string().unwrap().starts_with(&format!(r#"{{"code":{}"#, $code)));
			}
		}
	};
}
from_filters!(
	default,
	(1),
	"https://boylove.cc/home/api/cate/tp/1-0-2-1-1-0-1-2",
	1
);
from_filters!(
	basic_ongoing_safe_manga_2,
	(
		2,
		FilterValue::Select {
			id: "閱覽權限".into(),
			value: "一般".into()
		},
		FilterValue::Select {
			id: "連載狀態".into(),
			value: "連載中".into()
		},
		FilterValue::Select {
			id: "內容分級".into(),
			value: "清水".into()
		},
		FilterValue::MultiSelect {
			id: "標籤".into(),
			included: ["日漫".into()].into(),
			excluded: [].into()
		}
	),
	"https://boylove.cc/home/api/cate/tp/1-%E6%97%A5%E6%BC%AB-0-1-2-1-1-0",
	1
);
from_filters!(
	vip_completed_nsfw_manhwa_h_3,
	(
		3,
		FilterValue::Select {
			id: "閱覽權限".into(),
			value: "VIP".into()
		},
		FilterValue::Select {
			id: "連載狀態".into(),
			value: "已完結".into()
		},
		FilterValue::Select {
			id: "內容分級".into(),
			value: "有肉".into()
		},
		FilterValue::MultiSelect {
			id: "標籤".into(),
			included: ["韩漫".into(), "高H".into()].into(),
			excluded: [].into()
		}
	),
	"https://boylove.cc/home/api/cate/tp/1-%E9%9F%A9%E6%BC%AB+%E9%AB%98H-1-1-3-2-1-1",
	1
);
from_filters!(
	author,
	(
		1,
		FilterValue::Text {
			id: "author".into(),
			value: "소조금".into()
		}
	),
	"https://boylove.cc/home/api/searchk?keyword=%EC%86%8C%EC%A1%B0%EA%B8%88&type=1&pageNo=1",
	0
);
from_filters!(
	tag,
	(
		1,
		FilterValue::Select {
			id: "genre".into(),
			value: "韩漫".into()
		}
	),
	"https://boylove.cc/home/api/searchk?keyword=%E9%9F%A9%E6%BC%AB&type=1&pageNo=1",
	0
);

macro_rules! from_query {
	($name:ident, $keyword:literal, $page:literal, $expected_url:literal) => {
		paste! {
			#[aidoku_test]
			fn [<from_filters_ $name>]() {
				let url = Url::from_query_or_filters(Some($keyword), $page, &[]).unwrap();
				assert_eq!(url, $expected_url);
				assert!(url.request().unwrap().string().unwrap().starts_with(r#"{"code":0"#));
			}
		}
	};
}
from_query!(
	red_1,
	"紅",
	1,
	"https://boylove.cc/home/api/searchk?keyword=%E7%B4%85&type=1&pageNo=1"
);
from_query!(
	snake_2,
	"蛇",
	2,
	"https://boylove.cc/home/api/searchk?keyword=%E8%9B%87&type=1&pageNo=2"
);

#[aidoku_test]
fn abs() {
	assert_eq!(
		Url::Abs("/bookimages/img/20240605/7d14a38ef25968d00999dcc1999a97dd.webp"),
		"https://boylove.cc/bookimages/img/20240605/7d14a38ef25968d00999dcc1999a97dd.webp"
	);
}

#[aidoku_test]
fn manga() {
	assert_eq!(
		Url::manga("16904"),
		"https://boylove.cc/home/book/index/id/16904"
	);
}

impl Debug for Url<'_> {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		write!(f, "{self}")
	}
}

impl<S: AsRef<str>> PartialEq<S> for Url<'_> {
	fn eq(&self, other: &S) -> bool {
		self.to_string().as_str() == other.as_ref()
	}
}
