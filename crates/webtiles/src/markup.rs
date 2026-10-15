//! Plain text from the two markups webtiles sends text in.

/// The plain text of crawl's colour-tagged text, as `formatted_string::parse_string` reads it:
/// `<lightred>` and `</lightred>` set colours, and `<<` is a literal `<`.
pub fn plain(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '<' {
            plain.push(c);
        } else if chars.peek() == Some(&'<') {
            chars.next();
            plain.push('<');
        } else {
            // A colour tag runs to the next `>`; crawl drops an unclosed one too.
            for c in chars.by_ref() {
                if c == '>' {
                    break;
                }
            }
        }
    }
    plain
}

/// The plain text of a line of HTML from a webtiles text area: `<span>` tags dropped and the
/// entities `tileweb-text.cc` writes decoded.
pub fn plain_html(html: &str) -> String {
    let mut plain = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find(['<', '&']) {
        plain.push_str(&rest[..start]);
        rest = &rest[start..];
        if rest.starts_with('<') {
            rest = rest.find('>').map_or("", |end| &rest[end + 1..]);
        } else if let Some((entity, text)) =
            ENTITIES.iter().find(|(entity, _)| rest.starts_with(entity))
        {
            plain.push_str(text);
            rest = &rest[entity.len()..];
        } else {
            plain.push('&');
            rest = &rest[1..];
        }
    }
    plain.push_str(rest);
    plain
}

/// The entities `WebTextArea::send` escapes.
const ENTITIES: &[(&str, &str)] = &[
    ("&lt;", "<"),
    ("&gt;", ">"),
    ("&amp;", "&"),
    ("&quot;", "\""),
];
