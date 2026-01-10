use aidoku::{
    Result,
    imports::net::Request,
    FilterValue, Manga, MangaPageResult, 
    alloc::{String, Vec, format, string::ToString},
    helpers::uri::encode_uri_component,

    error,
};
use crate::net;
use crate::settings;
use crate::models;

pub const V4_API_URL: &str = "https://v4api.zaimanhua.com/app/v1";

/// Create a GET request, attaching auth token if Enhanced Mode is active.
pub fn get_api_request(url: &str) -> Result<Request> {
    if let Some(token) = settings::get_token() {
        // Only use auth if enhanced mode is enabled
        if settings::get_enhanced_mode() {
            net::auth_request(url, &token)
        } else {
            net::get_request(url)
        }
    } else {
        net::get_request(url)
    }
}

/// Search manga by keyword
pub fn search_by_keyword(keyword: &str, page: i32) -> Result<MangaPageResult> {
    let encoded = encode_uri_component(keyword);
    let url = format!(
        "{}/search/index?keyword={}&source=0&page={}&size=20",
        V4_API_URL, encoded, page
    );

    let response: models::ApiResponse<models::SearchData> = get_api_request(&url)?.json_owned()?;
    let data = response.data.ok_or_else(|| error!("Missing data"))?;
    
    let mut result = models::manga_list_from_search(data.list);
    let total = data.total.unwrap_or(0) as i32;
    result.has_next_page = (page * 20) < total;
    Ok(result)
}

/// Browse manga with filters (including optional rank mode)
pub fn browse_with_filters(filters: &[FilterValue], page: i32) -> Result<MangaPageResult> {
    let mut sort_type: Option<&str> = None;
    let mut zone: Option<&str> = None;
    let mut status: Option<&str> = None;
    let mut cate: Option<&str> = None;
    let mut theme: Option<&str> = None;
    let mut rank_mode: Option<&str> = None;
    
    for filter in filters {
        if let FilterValue::Select { id, value } = filter {
            match id.as_str() {
                "排序" => sort_type = Some(value.as_str()),
                "地区" => zone = Some(value.as_str()),
                "状态" => status = Some(value.as_str()),
                "受众" => cate = Some(value.as_str()),
                "题材" => theme = Some(value.as_str()),
                "榜单" => rank_mode = Some(value.as_str()),
                _ => {}
            }
        }
    }
    
    let url = match rank_mode {
        Some("1") => format!("{}/comic/rank/list?rank_type=0&by_time=0&page={}&size=20", V4_API_URL, page),
        Some("2") => format!("{}/comic/rank/list?rank_type=0&by_time=1&page={}&size=20", V4_API_URL, page),
        Some("3") => format!("{}/comic/rank/list?rank_type=0&by_time=2&page={}&size=20", V4_API_URL, page),
        Some("4") => format!("{}/comic/rank/list?rank_type=0&by_time=3&page={}&size=20", V4_API_URL, page),
        _ => format!(
            "{}/comic/filter/list?sortType={}&cate={}&status={}&zone={}&theme={}&page={}&size=20",
            V4_API_URL, 
            sort_type.unwrap_or("1"), 
            cate.unwrap_or("0"), 
            status.unwrap_or("0"), 
            zone.unwrap_or("0"), 
            theme.unwrap_or("0"), 
            page
        )
    };

    if rank_mode.is_some() {
        let response: models::ApiResponse<Vec<models::RankItem>> = get_api_request(&url)?.json_owned()?;
        let data = response.data.ok_or_else(|| error!("Missing data"))?;
        Ok(models::manga_list_from_ranks(data))
    } else {
        let response: models::ApiResponse<models::FilterData> = get_api_request(&url)?.json_owned()?;
        let data = response.data.ok_or_else(|| error!("Missing data"))?;
        Ok(models::manga_list_from_filter(data.comic_list))
    }
}

/// Search manga by author name (complex hybrid search)
pub fn search_by_author(author: &str, page: i32) -> Result<MangaPageResult> {
    let encoded = encode_uri_component(author);
    
    let author_matches = |manga_authors: &str| -> bool {
        if manga_authors.contains(author) {
            return true;
        }
        for part in manga_authors.split('/') {
            let trimmed = part.trim();
            if !trimmed.is_empty() && (trimmed.contains(author) || author.contains(trimmed)) {
                return true;
            }
        }
        false
    };
    
    let mut all_tag_ids: Vec<i64> = Vec::new();
    let mut keyword_manga: Vec<models::SearchItem> = Vec::new();
    let mut seen_authors: Vec<String> = Vec::new();
    
    // Step 1: Search for manga by author name
    let search_url = format!("{}/search/index?keyword={}&source=0&page=1&size=50", V4_API_URL, encoded);
    
    if let Ok(response) = get_api_request(&search_url)?.json_owned::<models::ApiResponse<models::SearchData>>()
        && let Some(data) = response.data {
            for item in data.list {
                let manga_authors = item.authors.as_deref().unwrap_or("");
                
                if author_matches(manga_authors) {
                    let author_key = manga_authors.to_string();
                    if !seen_authors.contains(&author_key) {
                        seen_authors.push(author_key);
                        let _ = collect_author_tags(item.id, &mut all_tag_ids);
                    }
                    keyword_manga.push(item);
                }
            }
        }
    
    // Step 2: Fallback core name search if no results
    if all_tag_ids.is_empty() && keyword_manga.is_empty() {
        let core_name = author.trim_start_matches('◎').trim_start_matches('@').trim_start_matches('◯');
        let short_core = if core_name.chars().count() >= 4 {
            core_name.chars().take(2).collect::<String>()
        } else {
            core_name.to_string()
        };
        
        for core in [core_name, short_core.as_str()] {
            if core.is_empty() || core == author || !all_tag_ids.is_empty() { continue; }
            
            let core_encoded = encode_uri_component(core);
            let core_url = format!("{}/search/index?keyword={}&source=0&page=1&size=30", V4_API_URL, core_encoded);
            
            if let Ok(response) = get_api_request(&core_url)?.json_owned::<models::ApiResponse<models::SearchData>>()
                && let Some(data) = response.data {
                    for item in data.list {
                        if !all_tag_ids.is_empty() { break; }
                        
                        let manga_authors = item.authors.as_deref().unwrap_or("");
                        if manga_authors.contains(core) {
                            let author_key = manga_authors.to_string();
                            if !seen_authors.contains(&author_key) {
                                seen_authors.push(author_key);
                                let _ = collect_author_tags(item.id, &mut all_tag_ids);
                            }
                            keyword_manga.push(item);
                        }
                    }
                }
        }
    }
    
    // Step 3: Use tag_ids to get complete works (parallel requests)
    let mut tag_manga: Vec<models::FilterItem> = Vec::new();
    let mut tag_total = 0i32;
    
    if !all_tag_ids.is_empty() {
        let tag_requests: Vec<_> = all_tag_ids.iter()
            .filter_map(|tid| {
                let furl = format!("{}/comic/filter/list?theme={}&page={}&size=100", V4_API_URL, tid, page);
                net::get_request(&furl).ok()
            })
            .collect();
        
        let tag_responses = Request::send_all(tag_requests);
        
        for resp_result in tag_responses {
            if let Ok(resp) = resp_result
                && let Ok(response) = resp.get_json_owned::<models::ApiResponse<models::FilterData>>()
                   && let Some(data) = response.data {
                        tag_total = tag_total.max(data.comic_list.len() as i32);
                        tag_manga.extend(data.comic_list);
                    }
        }
    }
    
    // Step 4: Merge and deduplicate
    let mut seen_ids: Vec<i64> = Vec::new();
    let mut final_manga: Vec<Manga> = Vec::new();
    
    for item in tag_manga {
        let id = item.id;
        if id > 0 && !seen_ids.contains(&id) {
            seen_ids.push(id);
            final_manga.push(item.into());
        }
    }
    
    for item in keyword_manga {
        let id = item.id;
        if id > 0 && !seen_ids.contains(&id) {
            seen_ids.push(id);
            final_manga.push(item.into());
        }
    }
    
    if !final_manga.is_empty() {
        let has_next = if tag_total > 0 { (page * 100) < tag_total } else { final_manga.len() >= 100 };
        return Ok(MangaPageResult { entries: final_manga, has_next_page: has_next });
    }
    
    Ok(MangaPageResult::default())
}

fn collect_author_tags(manga_id: i64, tag_ids: &mut Vec<i64>) -> Result<()> {
    let detail_url = format!("{}/comic/detail/{}?channel=android", V4_API_URL, manga_id);
    
    if let Ok(response) = get_api_request(&detail_url)?.json_owned::<models::ApiResponse<models::DetailData>>()
       && let Some(detail_data) = response.data
       && let Some(detail) = detail_data.data
       && let Some(authors) = detail.authors 
    {
        for author in authors {
            if let Some(tid) = author.tag_id 
                && tid > 0 && !tag_ids.contains(&tid)
            {
                tag_ids.push(tid);
            }
        }
    }
    Ok(())
}
