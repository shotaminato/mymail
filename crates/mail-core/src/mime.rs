use mail_parser::{Addr, Address, MessageParser};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedMail {
    pub from: String,
    pub subject: String,
    pub date: String,
    pub date_unix: Option<i64>,
    pub text: Option<String>,
    pub html: Option<String>,
}

pub fn parse_rfc822(raw: &[u8]) -> ParsedMail {
    let Some(msg) = MessageParser::default().parse(raw) else {
        return ParsedMail::default();
    };

    let from = msg
        .from()
        .and_then(Address::first)
        .map(format_addr)
        .unwrap_or_default();
    let subject = msg.subject().unwrap_or_default().to_string();
    let (date, date_unix) = msg
        .date()
        .map(|d| (d.to_rfc3339(), Some(d.to_timestamp())))
        .unwrap_or_else(|| (String::new(), None));

    let html = if msg.html_body_count() > 0 {
        msg.body_html(0).map(|s| s.into_owned())
    } else {
        None
    };
    let text = msg
        .body_text(0)
        .map(|s| s.into_owned())
        .or_else(|| html.as_ref().map(|h| strip_tags_rough(h)))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    ParsedMail {
        from,
        subject,
        date,
        date_unix,
        text,
        html,
    }
}

fn format_addr(addr: &Addr<'_>) -> String {
    match (&addr.name, &addr.address) {
        (Some(name), Some(email)) => format!("{name} <{email}>"),
        (_, Some(email)) => email.to_string(),
        (Some(name), None) => name.to_string(),
        _ => String::new(),
    }
}

fn strip_tags_rough(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAIN: &str = "From: Alice <alice@example.com>\r\n\
Subject: Hello\r\n\
Date: Tue, 1 Apr 2025 09:00:00 +0900\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
\r\n\
plain body\r\n";

    const JAPANESE: &str = "From: =?UTF-8?B?5bGx55Sw?= <taro@example.com>\r\n\
Subject: =?UTF-8?B?5Lya6K2w44Gu5Lu2?=\r\n\
Date: Tue, 1 Apr 2025 10:00:00 +0900\r\n\
MIME-Version: 1.0\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
Content-Transfer-Encoding: 8bit\r\n\
\r\n\
来週の会議は火曜です。全文検索テスト。\r\n";

    const HTML: &str = "From: News <news@example.com>\r\n\
Subject: HTML mail\r\n\
MIME-Version: 1.0\r\n\
Content-Type: text/html; charset=utf-8\r\n\
\r\n\
<html><body><h1>Hello</h1><script>alert(1)</script>\
<img src=\"https://evil.example/x.png\"></body></html>\r\n";

    #[test]
    fn parses_plain_english() {
        let parsed = parse_rfc822(PLAIN.as_bytes());
        assert_eq!(parsed.from, "Alice <alice@example.com>");
        assert_eq!(parsed.subject, "Hello");
        assert_eq!(parsed.text.as_deref(), Some("plain body"));
    }

    #[test]
    fn parses_japanese_encoded_headers() {
        let parsed = parse_rfc822(JAPANESE.as_bytes());
        assert!(parsed.from.contains("taro@example.com"));
        assert_eq!(parsed.subject, "会議の件");
        assert!(parsed
            .text
            .as_deref()
            .unwrap_or_default()
            .contains("全文検索テスト"));
    }

    #[test]
    fn parses_html_body() {
        let parsed = parse_rfc822(HTML.as_bytes());
        assert_eq!(parsed.subject, "HTML mail");
        let html = parsed.html.expect("html body");
        assert!(html.contains("<h1>Hello</h1>"));
        assert!(html.contains("evil.example"));
        let text = parsed.text.expect("text fallback");
        assert!(text.contains("Hello"));
    }
}
