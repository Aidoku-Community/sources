use aidoku::{MangaStatus, alloc::string::String};

pub fn parse_status(s: Option<&str>) -> MangaStatus {
    match s {
        Some("ONGOING") => MangaStatus::Ongoing,
        Some("COMPLETED") => MangaStatus::Completed,
        Some("DROPPED") | Some("CANCELLED") => MangaStatus::Cancelled,
        Some("HIATUS") => MangaStatus::Hiatus,
        _ => MangaStatus::Unknown,
    }
}

pub fn strip_html(html: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for ch in html.chars() {
        match ch {
            '<' => depth += 1,
            '>' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    String::from(out.trim())
}

