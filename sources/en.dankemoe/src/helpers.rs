use aidoku::alloc::string::String;

pub fn strip_html(html: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    let mut in_quote = false;
    let mut quote_char = '"';
    for ch in html.chars() {
        match ch {
            '"' | '\'' if depth > 0 && !in_quote => { in_quote = true; quote_char = ch; }
            c if in_quote && c == quote_char      => { in_quote = false; }
            '<' if !in_quote                      => depth += 1,
            '>' if depth > 0 && !in_quote         => depth -= 1,
            _ if depth == 0                        => out.push(ch),
            _                                      => {}
        }
    }
    String::from(out.trim())
}
