use aidoku::{
    FilterValue, Result,
    alloc::{String, format, string::ToString},
    bail, error,
    helpers::uri::{QueryParameters, encode_uri_component},
    imports::net::{HttpMethod, Request},
};
use alloc::vec::Vec;

use crate::net::Url::Filters;
use core::fmt::{Display, Formatter, Result as FmtResult};
use strum::{Display, EnumIs};

const API_URL: &str = "https://www.zerobyw33.com";

#[derive(Display, EnumIs)]
pub enum Url<'a> {
    #[strum(to_string = "/pc/pc/?{0}")]
    Filters(FiltersQuery),
    #[strum(to_string = "/pc/pc/?{0}")]
    Search(SearchQuery),
    #[strum(to_string = "/pc/details/?kuid={key}")]
    Manga { key: &'a str },
    #[strum(to_string = "/pc/view/index.php?zjid={key}")]
    Chapter { key: &'a str },
    #[strum(to_string = "/member.php?mod=logging&action=login")]
    Login,
}

impl Url<'_> {
    pub fn to_string(&self) -> Result<String> {
        let base_url = API_URL;
        Ok(format!("{base_url}{self}"))
    }

    pub fn request(&self, method: HttpMethod) -> Result<Request> {
        let url = self.to_string()?;
        let request = Request::new(url, method)?.header(
            "User-Agent",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
			 AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0.1 Safari/605.1.15",
        );
        Ok(request)
    }

    pub fn from_query_or_filters(
        query: Option<&str>,
        page: i32,
        filters: &[FilterValue],
    ) -> Result<Self> {
        if let Some(keyword) = query {
            let url = Self::Search(SearchQuery::new(keyword, page));
            return Ok(url);
        }

        let mut category_id = String::new();
        let mut jindu = String::new();
        let mut shuxing = String::new();
        let mut order = String::from("addtime");
        let mut dir = String::from("desc");

        for filter in filters {
            match filter {
                FilterValue::Select { id, value } => match id.as_str() {
                    "分类" => category_id = value.clone(),
                    "进度" => jindu = value.clone(),
                    "语言" => shuxing = value.clone(),
                    _ => (),
                },
                FilterValue::Sort {
                    id,
                    index,
                    ascending,
                } => match id.as_str() {
                    "排序" => {
                        dir = if *ascending {
                            "asc".to_string()
                        } else {
                            "desc".to_string()
                        };
                        match index {
                            0 => order = "addtime".to_string(),
                            1 => order = "views".to_string(),
                            2 => order = "favores".to_string(),
                            _ => bail!("Invalid index"),
                        }
                    }
                    _ => bail!("Invalid sort filter id:`{id}`"),
                },

                _ => bail!("Invalid filter:`{filter:?}`"),
            }
        }

        let filters_query = FiltersQuery::new(&category_id, &jindu, &shuxing, &dir, &order, page);
        Ok(Filters(filters_query))
    }
}

impl<'a> Url<'a> {
    pub const fn manga(key: &'a str) -> Self {
        Self::Manga { key }
    }
    pub const fn chapter(key: &'a str) -> Self {
        Self::Chapter { key }
    }
    pub const fn login() -> Self {
        Self::Login
    }
}

pub struct FiltersQuery(QueryParameters);

impl FiltersQuery {
    fn new(
        category_id: &str,
        jindu: &str,
        shuxing: &str,
        dir: &str,
        order: &str,
        page: i32,
    ) -> Self {
        let mut query = QueryParameters::new();

        if !category_id.is_empty() {
            query.push_encoded("category_id", Some(category_id));
        }

        if !jindu.is_empty() {
            query.push_encoded("jindu", Some(jindu));
        }

        if !shuxing.is_empty() {
            query.push_encoded("shuxing", Some(shuxing));
        }

        if !order.is_empty() {
            query.push_encoded("order", Some(order));
        }
        query.push_encoded("dir", Some(dir));
        query.push_encoded("page", Some(&*page.to_string()));

        Self(query)
    }
}

impl Display for FiltersQuery {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", self.0)
    }
}

pub struct SearchQuery(QueryParameters);

impl SearchQuery {
    fn new(keyword: &str, page: i32) -> Self {
        let mut query = QueryParameters::new();

        query.push_encoded("keyword", Some(keyword));
        query.push_encoded("page", Some(&*page.to_string()));

        Self(query)
    }
}
impl Display for SearchQuery {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", self.0)
    }
}

pub fn login(username: &str, password: &str) -> Result<bool> {
    let login_url = Url::login();
    let login_doc = login_url.request(HttpMethod::Get)?.html()?;

    let formhash = login_doc
        .select_first("input[name='formhash']")
        .ok_or_else(|| error!("formhash not found in form"))?
        .attr("value")
        .ok_or_else(|| error!("No formhash found"))?
        .to_string();

    let form = login_doc
        .select("form[action*='logging&action=login']")
        .ok_or_else(|| error!("formaction not found in form"))?
        .first()
        .ok_or_else(|| error!("No form action found"))?;
    let action = form
        .attr("action")
        .ok_or_else(|| error!("Action not found"))?
        .to_string();

    let loginhash = action
        .split("loginhash=")
        .nth(1)
        .and_then(|s| s.split('&').next())
        .ok_or_else(|| error!("loginhash not found"))?
        .to_string();
    if loginhash.is_empty() {
        return Err(error!("loginhash is empty"));
    }

    let post_url = if action.starts_with("http") {
        action
    } else {
        format!("{}/{}", API_URL, action)
    };

    let params = [
        ("formhash", formhash),
        ("referer", format!("{}/./", API_URL)),
        ("loginfield", "username".to_string()),
        ("username", username.to_string()),
        ("password", password.to_string()),
        ("cookietime", "2592000".to_string()),
        ("loginsubmit", "true".to_string()),
        ("questionid", "0".to_string()),
        ("answer", "".to_string()),
    ];

    let body = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, encode_uri_component(v)))
        .collect::<Vec<_>>()
        .join("&");

    let mut request = Request::new(post_url, HttpMethod::Post)?;
    request.set_header("Content-Type", "application/x-www-form-urlencoded");
    request.set_body(body.as_bytes()); // 假设 body 接受 &[u8]

    let response = request.send()?;

    let text = response.get_string()?;
    if text.contains("登录失败")
        || text.contains("密码错误")
        || text.contains("用户名不存在")
        || text.contains("error")
    {
        return Ok(false);
    }
    Ok(true)
}
