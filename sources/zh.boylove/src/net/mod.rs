use super::*;
use aidoku::{
	alloc::{format, string::ToString as _},
	error,
	imports::{defaults::defaults_get, net::Request},
};
use strum::Display;

#[derive(Display)]
#[strum(prefix = "https://boylove.cc")]
pub enum Url {
	#[strum(to_string = "/home/user/to{0}.html")]
	ChangeCharset(Charset),
}

impl Url {
	pub fn request(&self) -> Result<Request> {
		let request = Request::get(self.to_string())?;
		Ok(request)
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

#[cfg(test)]
mod test;
