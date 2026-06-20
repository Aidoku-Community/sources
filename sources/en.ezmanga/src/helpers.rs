use crate::{models::ApiSeriesItem, BASE_URL};
use aidoku::{
    Link, Manga, MangaStatus,
    alloc::{format, string::String},
};

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

fn parse_num_bytes(bytes: &[u8]) -> Option<i64> {
    let mut n = 0i64;
    for &b in bytes {
        if !b.is_ascii_digit() {
            return None;
        }
        n = n * 10 + (b - b'0') as i64;
    }
    Some(n)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let m = if m <= 2 { m + 9 } else { m - 3 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * m + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn parse_date(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 10 {
        return None;
    }
    let y = parse_num_bytes(&b[0..4])?;
    let mo = parse_num_bytes(&b[5..7])?;
    let d = parse_num_bytes(&b[8..10])?;
    let days = days_from_civil(y, mo, d);
    let secs = if b.len() >= 19 {
        let h = parse_num_bytes(&b[11..13])?;
        let mi = parse_num_bytes(&b[14..16])?;
        let se = parse_num_bytes(&b[17..19])?;
        h * 3600 + mi * 60 + se
    } else {
        0
    };
    Some(days * 86400 + secs)
}

pub fn item_to_manga(s: ApiSeriesItem) -> Manga {
    Manga {
        url: Some(format!("{}/series/{}", BASE_URL, s.slug)),
        key: s.slug,
        title: String::from(s.title.trim()),
        cover: if s.cover.is_empty() { None } else { Some(s.cover) },
        ..Default::default()
    }
}

pub fn item_to_link(s: ApiSeriesItem) -> Link {
    Link::from(item_to_manga(s))
}
