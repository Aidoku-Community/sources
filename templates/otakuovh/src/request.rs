use aidoku::{
	Result,
	imports::net::{Request, Response},
};

use crate::Params;

const USER_AGENT: &str = "Kastil-Dex-Parser";
const X_CLIENT_VERSION: &str = "113";
const HEADER_CONTENT_TYPE: &str = "content-type";
const CONTENT_TYPE_FORM: &str = "application/json";

pub trait InkRequest {
	fn prepared_headers(self, params: &Params) -> Result<Response>;
}

impl InkRequest for Request {
	fn prepared_headers(mut self, params: &Params) -> Result<Response> {
		self = self
			.header("referer", params.base_url.as_ref())
			.header("user-agent", USER_AGENT)
			.header("X-client-version", X_CLIENT_VERSION)
			.header(HEADER_CONTENT_TYPE, CONTENT_TYPE_FORM);

		let response = self.send()?;

		Ok(response)
	}
}

