//! Plain text from the markup webtiles sends text in.

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
