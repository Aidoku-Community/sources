use super::*;

impl KomiicSource {
	pub(super) fn empty_page() -> MangaPageResult {
		MangaPageResult {
			entries: Vec::new(),
			has_next_page: false,
		}
	}

	pub(super) fn listing(id: &str, name: &str) -> Listing {
		Listing {
			id: String::from(id),
			name: String::from(name),
			..Default::default()
		}
	}

	pub(super) fn category_listing(listing_id: &str, page: i32) -> Option<Result<MangaPageResult>> {
		let mut parts = listing_id.split(':');
		if parts.next()? != "category" {
			return None;
		}
		let category_id = parts.next()?;
		let order_by = parts.next().unwrap_or("DATE_UPDATED");
		Some(Self::comics_by_category(category_id, order_by, "", page))
	}

	fn offset(page: i32, limit: i32) -> i32 {
		if page <= 1 { 0 } else { (page - 1) * limit }
	}

	fn request(url: &str) -> Result<Request> {
		let mut request = Request::post(url)?;
		request.set_header("Accept", "application/json");
		request.set_header("Referer", REFERER_URL);
		request.set_header("User-Agent", USER_AGENT);
		request.set_header("Content-Type", "application/json");
		if let Some(token) = Self::auth_token() {
			let authorization = format!("Bearer {token}");
			request.set_header("Authorization", authorization.as_str());
		}
		Ok(request)
	}

	pub(super) fn post_json(url: &str, payload: Value) -> Result<Value> {
		let body = serde_json::to_string(&payload)?;
		let response = Self::request(url)?.body(body.as_bytes()).send()?;
		let status = response.status_code();
		if status != 200 {
			bail!("Komiic HTTP {status}");
		}
		let value: Value = response.get_json_owned()?;
		if let Some(errors) = value.get("errors").and_then(Value::as_array) {
			let message = errors
				.first()
				.and_then(|error| error.get("message"))
				.and_then(Value::as_str)
				.unwrap_or("GraphQL error");
			bail!("Komiic {message}");
		}
		Ok(value)
	}

	pub(super) fn query(payload: Value) -> Result<Value> {
		Self::post_json(QUERY_URL, payload)
	}

	pub(super) fn string_field(value: &Value, key: &str) -> Option<String> {
		value.get(key).and_then(Value::as_str).map(String::from)
	}

	fn names(value: &Value, key: &str) -> Option<Vec<String>> {
		let names = value
			.get(key)
			.and_then(Value::as_array)?
			.iter()
			.filter_map(|value| value.get("name").and_then(Value::as_str).map(String::from))
			.collect::<Vec<_>>();
		if names.is_empty() { None } else { Some(names) }
	}

	fn category_id(name: &str) -> Option<&'static str> {
		match name.trim() {
			"全部" => Some("0"),
			"愛情" | "爱情" => Some("1"),
			"神鬼" => Some("3"),
			"校園" | "校园" => Some("4"),
			"搞笑" => Some("5"),
			"生活" => Some("6"),
			"懸疑" | "悬疑" => Some("7"),
			"冒險" | "冒险" => Some("8"),
			"職場" | "职场" => Some("10"),
			"魔幻" => Some("11"),
			"後宮" | "后宫" => Some("2"),
			"魔法" => Some("12"),
			"格鬥" | "格斗" => Some("13"),
			"宅男" => Some("14"),
			"勵志" | "励志" => Some("15"),
			"耽美" => Some("16"),
			"科幻" => Some("17"),
			"百合" => Some("18"),
			"治癒" | "治愈" => Some("19"),
			"萌系" => Some("20"),
			"熱血" | "热血" => Some("21"),
			"競技" | "竞技" => Some("22"),
			"推理" => Some("23"),
			"雜誌" | "杂志" => Some("24"),
			"偵探" | "侦探" => Some("25"),
			"偽娘" | "伪娘" => Some("26"),
			"美食" => Some("27"),
			"恐怖" => Some("9"),
			"四格" => Some("28"),
			"社會" | "社会" => Some("31"),
			"歷史" | "历史" => Some("32"),
			"戰爭" | "战争" => Some("33"),
			"舞蹈" => Some("34"),
			"武俠" | "武侠" => Some("35"),
			"機戰" | "机战" => Some("36"),
			"音樂" | "音乐" => Some("37"),
			"體育" | "体育" => Some("40"),
			"黑道" => Some("42"),
			_ => None,
		}
	}

	pub(super) fn category_filter_value(value: &str) -> Option<String> {
		let value = value.trim();
		match value {
			"0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "10" | "11" | "12"
			| "13" | "14" | "15" | "16" | "17" | "18" | "19" | "20" | "21" | "22" | "23" | "24"
			| "25" | "26" | "27" | "28" | "31" | "32" | "33" | "34" | "35" | "36" | "37" | "40"
			| "42" => Some(String::from(value)),
			_ => Self::category_id(value).map(String::from),
		}
	}

	fn sort_order(index: i32) -> Option<&'static str> {
		if index < 0 {
			return None;
		}
		SORT_ORDER_IDS.get(index as usize).copied()
	}

	pub(super) fn parse_search_filters(
		query: Option<&str>,
		filters: &[FilterValue],
	) -> (String, String, String, Option<String>, Option<String>) {
		let mut order_by = String::from("DATE_UPDATED");
		let mut status = String::new();
		let mut category = String::from("0");
		let mut keyword = query
			.map(str::trim)
			.filter(|value| !value.is_empty())
			.map(String::from);
		let mut author = None;

		for filter in filters {
			match filter {
				FilterValue::Sort { id, index, .. } => {
					if id.as_str() == "sort"
						&& let Some(order) = Self::sort_order(*index)
					{
						order_by = String::from(order);
					}
				}
				FilterValue::Text { id, value } | FilterValue::Select { id, value } => {
					let value = value.trim();
					if value.is_empty() {
						continue;
					}
					match id.as_str() {
						"author" => author = Some(String::from(value)),
						"status" => status = String::from(value),
						"category" => {
							if let Some(id) = Self::category_filter_value(value) {
								category = id;
							}
						}
						"genre" => {
							if let Some(id) = Self::category_filter_value(value) {
								category = id;
							} else {
								keyword = Some(String::from(value));
							}
						}
						_ => {}
					}
				}
				_ => {}
			}
		}

		(order_by, status, category, keyword, author)
	}

	fn manga_status(value: &Value) -> MangaStatus {
		match value
			.get("status")
			.and_then(Value::as_str)
			.unwrap_or_default()
		{
			"ONGOING" => MangaStatus::Ongoing,
			"END" => MangaStatus::Completed,
			_ => MangaStatus::Unknown,
		}
	}

	fn parse_manga(value: &Value, minimal: bool) -> Manga {
		let key = Self::string_field(value, "id").unwrap_or_default();
		let title = Self::string_field(value, "title").unwrap_or_else(|| key.clone());
		let url = Some(format!("{BASE_URL}/comic/{key}"));
		let mut manga = Manga {
			key,
			title,
			cover: Self::string_field(value, "imageUrl"),
			url,
			status: Self::manga_status(value),
			content_rating: ContentRating::Safe,
			viewer: Viewer::RightToLeft,
			..Default::default()
		};
		if !minimal {
			manga.authors = Self::names(value, "authors");
			manga.description = Self::string_field(value, "description")
				.filter(|description| !description.trim().is_empty());
			manga.tags = Self::names(value, "categories");
		}
		manga
	}

	fn manga_array<'a>(value: &'a Value, path: &[&str]) -> Result<&'a Vec<Value>> {
		let mut current = value;
		for key in path {
			current = current
				.get(*key)
				.ok_or_else(|| error!("Komiic missing {key}"))?;
		}
		current
			.as_array()
			.ok_or_else(|| error!("Komiic response is not a list"))
	}

	fn parse_manga_list(value: &Value, path: &[&str]) -> Result<Vec<Manga>> {
		Ok(Self::parse_manga_list_with_raw_count(value, path)?.0)
	}

	pub(super) fn parse_manga_list_with_raw_count(
		value: &Value,
		path: &[&str],
	) -> Result<(Vec<Manga>, usize)> {
		Self::parse_manga_list_with_raw_count_and_mode(value, path, true)
	}

	pub(super) fn parse_manga_list_with_raw_count_and_mode(
		value: &Value,
		path: &[&str],
		minimal: bool,
	) -> Result<(Vec<Manga>, usize)> {
		let values = Self::manga_array(value, path)?;
		let raw_count = values.len();
		let entries = values
			.iter()
			.map(|value| Self::parse_manga(value, minimal))
			.collect::<Vec<_>>();
		Ok((Self::deduplicate_manga(entries), raw_count))
	}

	fn deduplicate_manga(entries: Vec<Manga>) -> Vec<Manga> {
		let mut keys = Vec::new();
		let mut deduplicated = Vec::new();
		for entry in entries {
			if keys.iter().any(|key| key == &entry.key) {
				continue;
			}
			keys.push(entry.key.clone());
			deduplicated.push(entry);
		}
		deduplicated
	}

	fn comic_list_query(operation_name: &str, order_by: &str, page: i32, fields: &str) -> Value {
		json!({
			"operationName": operation_name,
			"variables": {
				"pagination": {
					"limit": PAGE_SIZE,
					"offset": Self::offset(page, PAGE_SIZE),
					"orderBy": order_by,
					"status": "",
					"asc": true
				}
			},
			"query": format!(
				"query {operation_name}($pagination: Pagination!) {{ {operation_name}(pagination: $pagination) {{ {fields} }} }}"
			)
		})
	}

	fn get_comic_list_with_fields(
		operation_name: &str,
		order_by: &str,
		page: i32,
		fields: &str,
		minimal: bool,
	) -> Result<MangaPageResult> {
		let json = Self::query(Self::comic_list_query(
			operation_name,
			order_by,
			page,
			fields,
		))?;
		let (entries, raw_count) = Self::parse_manga_list_with_raw_count_and_mode(
			&json,
			&["data", operation_name],
			minimal,
		)?;
		Ok(MangaPageResult {
			has_next_page: raw_count == PAGE_SIZE as usize,
			entries,
		})
	}

	pub(super) fn get_comic_list(
		operation_name: &str,
		order_by: &str,
		page: i32,
	) -> Result<MangaPageResult> {
		Self::get_comic_list_with_fields(operation_name, order_by, page, COMIC_FIELDS, true)
	}

	pub(super) fn get_home_comic_list(
		operation_name: &str,
		order_by: &str,
		page: i32,
	) -> Result<MangaPageResult> {
		Self::get_comic_list_with_fields(operation_name, order_by, page, HOME_COMIC_FIELDS, false)
	}

	pub(super) fn comics_by_category(
		category_id: &str,
		order_by: &str,
		status: &str,
		page: i32,
	) -> Result<MangaPageResult> {
		let category_id = if category_id == "0" {
			json!([])
		} else {
			json!([category_id])
		};
		let json = Self::query(json!({
			"operationName": "comicByCategories",
			"variables": {
				"categoryId": category_id,
				"pagination": {
					"limit": CATEGORY_PAGE_SIZE,
					"offset": Self::offset(page, CATEGORY_PAGE_SIZE),
					"orderBy": order_by,
					"asc": false,
					"status": status
				}
			},
			"query": format!(
				"query comicByCategories($categoryId: [ID!]!, $pagination: Pagination!) {{ \
					comicByCategories(categoryId: $categoryId, pagination: $pagination) {{ \
						{COMIC_FIELDS} \
					}} \
				}}"
			)
		}))?;
		let (entries, raw_count) =
			Self::parse_manga_list_with_raw_count(&json, &["data", "comicByCategories"])?;
		Ok(MangaPageResult {
			has_next_page: raw_count == CATEGORY_PAGE_SIZE as usize,
			entries,
		})
	}

	pub(super) fn search(keyword: String) -> Result<MangaPageResult> {
		let json = Self::query(json!({
			"operationName": "searchComicAndAuthorQuery",
			"variables": { "keyword": keyword },
			"query": format!(
				"query searchComicAndAuthorQuery($keyword: String!) {{ searchComicsAndAuthors(keyword: $keyword) {{ comics {{ {COMIC_FIELDS} }} authors {{ id name chName enName wikiLink comicCount views __typename }} __typename }} }}"
			)
		}))?;
		let entries = Self::parse_manga_list(&json, &["data", "searchComicsAndAuthors", "comics"])?;
		Ok(MangaPageResult {
			entries,
			has_next_page: false,
		})
	}

	fn author_matches(value: &Value, keyword: &str) -> bool {
		["name", "chName", "enName"].iter().any(|key| {
			value
				.get(*key)
				.and_then(Value::as_str)
				.map(str::trim)
				.filter(|value| !value.is_empty())
				.is_some_and(|value| value == keyword)
		})
	}

	fn search_author_ids(keyword: &str) -> Result<Vec<String>> {
		let keyword = keyword.trim();
		if keyword.is_empty() {
			return Ok(Vec::new());
		}
		let json = Self::query(json!({
			"operationName": "searchComicAndAuthorQuery",
			"variables": { "keyword": keyword },
			"query": "query searchComicAndAuthorQuery($keyword: String!) { searchComicsAndAuthors(keyword: $keyword) { authors { id name chName enName comicCount __typename } __typename } }"
		}))?;
		let values = json
			.get("data")
			.and_then(|value| value.get("searchComicsAndAuthors"))
			.and_then(|value| value.get("authors"))
			.and_then(Value::as_array)
			.ok_or_else(|| error!("Komiic missing authors"))?;
		let mut ids = Vec::new();
		for value in values {
			if Self::author_matches(value, keyword)
				&& let Some(id) = value.get("id").and_then(Value::as_str)
				&& !ids.iter().any(|existing| existing == id)
			{
				ids.push(String::from(id));
			}
		}
		Ok(ids)
	}

	fn comics_by_author_id(author_id: &str) -> Result<Vec<Manga>> {
		let json = Self::query(json!({
			"operationName": "comicsByAuthor",
			"variables": { "authorId": author_id },
			"query": format!(
				"query comicsByAuthor($authorId: ID!) {{ getComicsByAuthor(authorId: $authorId) {{ {COMIC_FIELDS} }} }}"
			)
		}))?;
		Self::parse_manga_list(&json, &["data", "getComicsByAuthor"])
	}

	pub(super) fn search_by_author(keyword: String, page: i32) -> Result<MangaPageResult> {
		if page > 1 {
			return Ok(Self::empty_page());
		}
		let mut entries = Vec::new();
		for id in Self::search_author_ids(keyword.as_str())? {
			entries.extend(Self::comics_by_author_id(id.as_str())?);
		}
		Ok(MangaPageResult {
			entries: Self::deduplicate_manga(entries),
			has_next_page: false,
		})
	}

	fn personalized_suggestions() -> Result<Vec<String>> {
		if Self::auth_token().is_none() {
			bail!("请先登录 Komiic");
		}
		let json = Self::query(json!({
			"operationName": "personalizedSuggestions",
			"variables": {
				"limit": RECOMMENDATION_PAGE_SIZE,
				"excludeRead": true,
				"excludeFavorites": true
			},
			"query": "query personalizedSuggestions($limit: Int, $contextComicId: ID, $excludeRead: Boolean, $excludeFavorites: Boolean, $contentType: ContentType) { personalizedSuggestions(limit: $limit, contextComicId: $contextComicId, excludeRead: $excludeRead, excludeFavorites: $excludeFavorites, contentType: $contentType) { comicId __typename } }"
		}))?;
		let values = json
			.get("data")
			.and_then(|value| value.get("personalizedSuggestions"))
			.and_then(Value::as_array)
			.ok_or_else(|| error!("Komiic missing recommendations"))?;
		let mut ids = Vec::new();
		for value in values {
			if let Some(id) = value.get("comicId").and_then(Value::as_str)
				&& !ids.iter().any(|existing| existing == id)
			{
				ids.push(String::from(id));
			}
		}
		Ok(ids)
	}

	fn comics_by_ids(comic_ids: Vec<String>) -> Result<Vec<Manga>> {
		if comic_ids.is_empty() {
			return Ok(Vec::new());
		}
		let json = Self::query(json!({
			"operationName": "comicByIds",
			"variables": { "comicIds": comic_ids.clone() },
			"query": format!(
				"query comicByIds($comicIds: [ID]!) {{ comicByIds(comicIds: $comicIds) {{ {COMIC_FIELDS} }} }}"
			)
		}))?;
		let mut entries = Self::parse_manga_list(&json, &["data", "comicByIds"])?;
		let mut ordered = Vec::new();
		for key in comic_ids {
			if let Some(index) = entries.iter().position(|entry| entry.key == key) {
				ordered.push(entries.remove(index));
			}
		}
		Ok(ordered)
	}

	pub(super) fn recommendations(page: i32) -> Result<MangaPageResult> {
		if page > 1 {
			return Ok(Self::empty_page());
		}
		let entries = Self::comics_by_ids(Self::personalized_suggestions()?)?;
		Ok(MangaPageResult {
			entries,
			has_next_page: false,
		})
	}

	pub(super) fn comic_by_id(id: String) -> Result<Option<Manga>> {
		let json = Self::query(json!({
			"operationName": "comicById",
			"variables": { "comicId": id },
			"query": format!(
				"query comicById($comicId: ID!) {{ comicById(comicId: $comicId) {{ {DETAIL_COMIC_FIELDS} }} }}"
			)
		}))?;
		if let Some(comic) = json.get("data").and_then(|value| value.get("comicById"))
			&& !comic.is_null()
		{
			Ok(Some(Self::parse_manga(comic, false)))
		} else {
			Ok(None)
		}
	}
}
