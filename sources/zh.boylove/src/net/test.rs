use super::*;
use aidoku_test::aidoku_test;
use core::fmt::{Debug, Formatter, Result};
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

impl Debug for Url {
	fn fmt(&self, f: &mut Formatter<'_>) -> Result {
		write!(f, "{self}")
	}
}

impl<S: AsRef<str>> PartialEq<S> for Url {
	fn eq(&self, other: &S) -> bool {
		self.to_string().as_str() == other.as_ref()
	}
}
