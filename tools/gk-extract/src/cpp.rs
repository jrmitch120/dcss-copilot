//! Just enough of C++ to read crawl's enums and data tables: the `TAG_MAJOR_VERSION`
//! conditionals, function-like macros that build table entries, and tokens.
//!
//! Anything this reader doesn't understand is an error rather than a guess, so a crawl change
//! that breaks it fails the extraction instead of producing a wrong bundle.

use std::collections::HashMap;

use crate::ExtractError;

/// The `#ifdef` names a webtiles server's crawl build defines, besides `TAG_MAJOR_VERSION`.
const WEBTILES_DEFINES: &[&str] = &["USE_TILE", "USE_TILE_WEB"];

/// One C++ token. Adjacent string literals are joined, as the compiler joins them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Ident(String),
    Number(String),
    Str(String),
    Char(String),
    Punct(char),
}

/// A source file as a webtiles build of one crawl version compiles it.
#[derive(Debug)]
pub struct Source {
    file: String,
    tokens: Vec<Token>,
}

impl Source {
    /// Preprocesses `text` for a build with this `TAG_MAJOR_VERSION`, expands the file's own
    /// function-like macros and tokenizes the result. `file` names it in errors.
    pub fn parse(file: &str, text: &str, tag_major: i64) -> Result<Self, ExtractError> {
        let (code, macros) = preprocess(file, text, tag_major)?;
        let tokens = expand(file, &tokenize(file, &code)?, &macros)?;
        Ok(Self {
            file: file.to_owned(),
            tokens: join_strings(tokens),
        })
    }

    /// The enumerators of `enum <name>`, with their values, in declaration order.
    pub fn enumeration(&self, name: &str) -> Result<Vec<(String, i64)>, ExtractError> {
        let start = self
            .find(&[Token::Ident("enum".into()), Token::Ident(name.into())])
            .ok_or_else(|| self.missing(&format!("enum {name}")))?;
        let open = (start..self.tokens.len())
            .find(|&i| self.tokens[i] == Token::Punct('{'))
            .ok_or_else(|| self.missing(&format!("the body of enum {name}")))?;
        let close = self.matching_brace(open)?;

        let mut values: Vec<(String, i64)> = Vec::new();
        let mut next = 0;
        for enumerator in self.tokens[open + 1..close].split(|t| *t == Token::Punct(',')) {
            let (ident, init) = match enumerator {
                [] => continue,
                [Token::Ident(ident)] => (ident, None),
                [Token::Ident(ident), Token::Punct('='), init @ ..] => (ident, Some(init)),
                _ => return Err(self.unreadable(&format!("an enumerator of enum {name}"))),
            };
            let value = match init {
                None => next,
                Some(init) => self.constant(init, &values).ok_or_else(|| {
                    self.unreadable(&format!("the value of {ident} in enum {name}"))
                })?,
            };
            values.push((ident.clone(), value));
            next = value + 1;
        }
        Ok(values)
    }

    /// The `(identifier, name)` entries of the array initializer `<name>[] = { ... }`: each
    /// entry that starts `{ IDENT, "name"`.
    pub fn table(&self, name: &str) -> Result<Vec<(String, String)>, ExtractError> {
        let entries: Vec<_> = self
            .initializer(name)?
            .windows(4)
            .filter_map(|window| match window {
                [
                    Token::Punct('{'),
                    Token::Ident(ident),
                    Token::Punct(','),
                    Token::Str(text),
                ] => Some((ident.clone(), text.clone())),
                _ => None,
            })
            .collect();
        if entries.is_empty() {
            return Err(self.unreadable(&format!("the entries of {name}")));
        }
        Ok(entries)
    }

    /// Each entry of the array initializer `<name>[] = { ... }`, split into its fields: the
    /// comma-separated parts of a braced entry, or the arguments of an entry built by calling a
    /// function, as `_mon_hex_zap(ZAP_SLOW, BEAM_SLOW)`, with the function's name as the first.
    pub fn entries(&self, name: &str) -> Result<Vec<Vec<Vec<Token>>>, ExtractError> {
        let mut entries = Vec::new();
        for entry in split_top_level(self.initializer(name)?) {
            let fields = match entry.as_slice() {
                [] => continue,
                [Token::Punct('{'), inner @ .., Token::Punct('}')] => split_top_level(inner),
                [
                    Token::Ident(function),
                    Token::Punct('('),
                    inner @ ..,
                    Token::Punct(')'),
                ] => {
                    let mut fields = vec![vec![Token::Ident(function.clone())]];
                    fields.extend(split_top_level(inner));
                    fields
                }
                _ => return Err(self.unreadable(&format!("an entry of {name}"))),
            };
            entries.push(fields);
        }
        if entries.is_empty() {
            return Err(self.unreadable(&format!("the entries of {name}")));
        }
        Ok(entries)
    }

    /// The tokens inside the braces of the initializer `<name>[] = { ... }` or
    /// `<name> = { ... }`, as a table or a `static const map` is written.
    pub fn initializer(&self, name: &str) -> Result<&[Token], ExtractError> {
        let array = [
            Token::Ident(name.into()),
            Token::Punct('['),
            Token::Punct(']'),
            Token::Punct('='),
            Token::Punct('{'),
        ];
        let open = match self.find(&array) {
            Some(start) => start + 4,
            None => {
                let value = [
                    Token::Ident(name.into()),
                    Token::Punct('='),
                    Token::Punct('{'),
                ];
                self.find(&value)
                    .ok_or_else(|| self.missing(&format!("the table {name}")))?
                    + 2
            }
        };
        let close = self.matching_brace(open)?;
        Ok(&self.tokens[open + 1..close])
    }

    /// The `(identifier, name)` pairs of `case IDENT: return "name";` in the body of the
    /// function `<name>(...)`. Cases that fall through to the same return all get its name, and
    /// an identifier's first case wins.
    pub fn switch_names(&self, name: &str) -> Result<Vec<(String, String)>, ExtractError> {
        let start = (0..self.tokens.len())
            .find(|&i| self.is_definition_of(i, name))
            .ok_or_else(|| self.missing(&format!("the function {name}")))?;
        let open = (start..self.tokens.len())
            .find(|&i| self.tokens[i] == Token::Punct('{'))
            .ok_or_else(|| self.missing(&format!("the body of {name}")))?;
        let body = &self.tokens[open + 1..self.matching_brace(open)?];

        let mut names: Vec<(String, String)> = Vec::new();
        let mut pending: Vec<String> = Vec::new();
        let mut i = 0;
        while i < body.len() {
            match &body[i..] {
                [
                    Token::Ident(case),
                    Token::Ident(ident),
                    Token::Punct(':'),
                    ..,
                ] if case == "case" => {
                    pending.push(ident.clone());
                    i += 3;
                }
                [Token::Ident(ret), Token::Str(text), Token::Punct(';'), ..] if ret == "return" => {
                    for ident in pending.drain(..) {
                        if !names.iter().any(|(seen, _)| *seen == ident) {
                            names.push((ident, text.clone()));
                        }
                    }
                    i += 3;
                }
                [Token::Ident(word), ..] if word == "default" => {
                    pending.clear();
                    i += 1;
                }
                _ => i += 1,
            }
        }
        if names.is_empty() {
            return Err(self.unreadable(&format!("the cases of {name}")));
        }
        Ok(names)
    }

    /// `name ( ... ) {` starts at `i`, with no `;` between: a definition, not a call.
    fn is_definition_of(&self, i: usize, name: &str) -> bool {
        if self.tokens[i] != Token::Ident(name.into())
            || self.tokens.get(i + 1) != Some(&Token::Punct('('))
        {
            return false;
        }
        let Some(close) = (i + 1..self.tokens.len()).find(|&j| self.tokens[j] == Token::Punct(')'))
        else {
            return false;
        };
        self.tokens.get(close + 1) == Some(&Token::Punct('{'))
    }

    /// A constant initializer: a literal, a negative literal or an earlier enumerator.
    fn constant(&self, init: &[Token], earlier: &[(String, i64)]) -> Option<i64> {
        match init {
            [Token::Number(n)] => number(n),
            [Token::Punct('-'), Token::Number(n)] => number(n).map(|n| -n),
            [Token::Ident(ident)] => earlier
                .iter()
                .rev()
                .find(|(seen, _)| seen == ident)
                .map(|(_, value)| *value),
            _ => None,
        }
    }

    fn find(&self, pattern: &[Token]) -> Option<usize> {
        self.tokens
            .windows(pattern.len())
            .position(|window| window == pattern)
    }

    fn matching_brace(&self, open: usize) -> Result<usize, ExtractError> {
        let mut depth = 0usize;
        for (i, token) in self.tokens.iter().enumerate().skip(open) {
            match token {
                Token::Punct('{') => depth += 1,
                Token::Punct('}') => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(i);
                    }
                }
                _ => {}
            }
        }
        Err(self.unreadable("an unclosed brace"))
    }

    fn missing(&self, what: &str) -> ExtractError {
        ExtractError::Missing {
            file: self.file.clone(),
            what: what.to_owned(),
        }
    }

    fn unreadable(&self, what: &str) -> ExtractError {
        ExtractError::Unreadable {
            file: self.file.clone(),
            what: what.to_owned(),
        }
    }
}

/// `tokens` split at the commas outside any brackets. A `<` straight after a `..._calculator`
/// opens template arguments, the only angle brackets crawl's tables nest commas in.
pub fn split_top_level(tokens: &[Token]) -> Vec<Vec<Token>> {
    let mut parts = vec![Vec::new()];
    // The closing bracket each open one waits for.
    let mut open: Vec<char> = Vec::new();
    let mut previous: Option<&Token> = None;
    for token in tokens {
        match token {
            Token::Punct('{') => open.push('}'),
            Token::Punct('(') => open.push(')'),
            Token::Punct('[') => open.push(']'),
            Token::Punct('<') if matches!(previous, Some(Token::Ident(ident)) if ident.ends_with("_calculator")) =>
            {
                open.push('>');
            }
            Token::Punct(c) if open.last() == Some(c) => {
                open.pop();
            }
            Token::Punct(',') if open.is_empty() => {
                parts.push(Vec::new());
                previous = Some(token);
                continue;
            }
            _ => {}
        }
        if let Some(part) = parts.last_mut() {
            part.push(token.clone());
        }
        previous = Some(token);
    }
    if parts.last().is_some_and(Vec::is_empty) {
        parts.pop();
    }
    parts
}

/// The integer object-like macros a file defines, such as `#define LOS_RADIUS 8`.
pub fn integer_defines(text: &str) -> HashMap<String, i64> {
    strip_comments(text)
        .lines()
        .filter_map(|line| {
            let mut words = line.trim_start().strip_prefix('#')?.split_whitespace();
            let (Some("define"), Some(name), Some(value), None) =
                (words.next(), words.next(), words.next(), words.next())
            else {
                return None;
            };
            Some((name.to_owned(), number(value)?))
        })
        .collect()
}

/// A literal integer, as written: decimal or hex, with any suffix.
pub fn number(text: &str) -> Option<i64> {
    let digits = text.trim_end_matches(['u', 'U', 'l', 'L']);
    match digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        Some(hex) => i64::from_str_radix(hex, 16).ok(),
        None => digits.parse().ok(),
    }
}

/// A function-like macro: its parameters and its body's tokens.
struct Macro {
    params: Vec<String>,
    body: Vec<Token>,
}

/// One open `#if` group: whether its enclosing code is live, whether a branch has been taken,
/// and whether the current branch is live.
struct Group {
    outer: bool,
    taken: bool,
    live: bool,
}

/// Keeps the live lines for this build, without comments, and collects function-like macros.
fn preprocess(
    file: &str,
    text: &str,
    tag_major: i64,
) -> Result<(String, HashMap<String, Macro>), ExtractError> {
    let text = strip_comments(text).replace("\\\n", " ");
    let mut code = String::new();
    let mut macros = HashMap::new();
    let mut groups: Vec<Group> = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let live = groups.last().is_none_or(|group| group.live);
        let Some(directive) = line.trim_start().strip_prefix('#') else {
            if live {
                code.push_str(line);
            }
            code.push('\n');
            continue;
        };
        code.push('\n');
        let directive = directive.trim_start();
        let (word, rest) = directive
            .split_once(char::is_whitespace)
            .unwrap_or((directive, ""));
        let rest = rest.trim();
        let condition = |expr: &str| {
            condition(expr, tag_major).ok_or_else(|| ExtractError::Unreadable {
                file: file.to_owned(),
                what: format!("the condition on line {}: #{directive}", number + 1),
            })
        };
        match word {
            "if" | "ifdef" | "ifndef" => {
                // A dead group's conditions are never read, as the compiler never reads them.
                let holds = match word {
                    _ if !live => false,
                    "if" => condition(rest)?,
                    "ifdef" => is_defined(rest),
                    _ => !is_defined(rest),
                };
                groups.push(Group {
                    outer: live,
                    taken: holds,
                    live: live && holds,
                });
            }
            "elif" | "else" | "endif" => {
                let Some(group) = groups.last_mut() else {
                    return Err(ExtractError::Unreadable {
                        file: file.to_owned(),
                        what: format!("an unmatched #{word} on line {}", number + 1),
                    });
                };
                match word {
                    "endif" => {
                        groups.pop();
                    }
                    "else" => {
                        group.live = group.outer && !group.taken;
                        group.taken = true;
                    }
                    _ => {
                        let holds = group.outer && !group.taken && condition(rest)?;
                        group.live = group.outer && holds;
                        group.taken |= holds;
                    }
                }
            }
            "define" if live => {
                if let Some((name, mac)) = function_macro(file, rest)? {
                    macros.insert(name, mac);
                }
            }
            // Macros are expanded once the whole file is read, so `#undef` is not honoured: a
            // table's macro is undefined straight after the table that uses it.
            _ => {}
        }
    }
    Ok((code, macros))
}

/// Whether `#ifdef name` holds in a webtiles build.
fn is_defined(name: &str) -> bool {
    name == "TAG_MAJOR_VERSION" || WEBTILES_DEFINES.contains(&name)
}

/// Evaluates `TAG_MAJOR_VERSION <op> <n>`, the only `#if` crawl's data files use.
fn condition(expr: &str, tag_major: i64) -> Option<bool> {
    let mut words = expr.split_whitespace();
    let (Some("TAG_MAJOR_VERSION"), Some(op), Some(n), None) =
        (words.next(), words.next(), words.next(), words.next())
    else {
        return None;
    };
    let n: i64 = n.parse().ok()?;
    Some(match op {
        "==" => tag_major == n,
        "!=" => tag_major != n,
        ">" => tag_major > n,
        ">=" => tag_major >= n,
        "<" => tag_major < n,
        "<=" => tag_major <= n,
        _ => return None,
    })
}

/// A `#define NAME(params) body`, or `None` for an object-like macro, which tables don't use.
fn function_macro(file: &str, define: &str) -> Result<Option<(String, Macro)>, ExtractError> {
    let name_end = define
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(define.len());
    let (name, rest) = define.split_at(name_end);
    let Some(rest) = rest.strip_prefix('(') else {
        return Ok(None);
    };
    let unreadable = || ExtractError::Unreadable {
        file: file.to_owned(),
        what: format!("the macro {name}"),
    };
    let (params, body) = rest.split_once(')').ok_or_else(unreadable)?;
    let params = params.split(',').map(|p| p.trim().to_owned()).collect();
    let body = tokenize(file, body)?;
    Ok(Some((name.to_owned(), Macro { params, body })))
}

/// Replaces each use of a function-like macro with its body, substituting the arguments and
/// pasting tokens joined by `##`.
fn expand(
    file: &str,
    tokens: &[Token],
    macros: &HashMap<String, Macro>,
) -> Result<Vec<Token>, ExtractError> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        let mac = match &tokens[i] {
            Token::Ident(name) if tokens.get(i + 1) == Some(&Token::Punct('(')) => macros.get(name),
            _ => None,
        };
        let Some(mac) = mac else {
            out.push(tokens[i].clone());
            i += 1;
            continue;
        };
        let (args, after) = arguments(tokens, i + 1).ok_or_else(|| ExtractError::Unreadable {
            file: file.to_owned(),
            what: format!("a use of the macro {:?}", tokens[i]),
        })?;
        let mut body: Vec<Token> = Vec::new();
        for token in &mac.body {
            match token {
                Token::Ident(ident) => match mac.params.iter().position(|p| p == ident) {
                    Some(arg) => body.extend(args.get(arg).cloned().unwrap_or_default()),
                    None => body.push(token.clone()),
                },
                _ => body.push(token.clone()),
            }
        }
        out.extend(paste(body));
        i = after;
    }
    Ok(out)
}

/// The comma-separated arguments of the call whose `(` is at `open`, and the index after its `)`.
fn arguments(tokens: &[Token], open: usize) -> Option<(Vec<Vec<Token>>, usize)> {
    let mut args = vec![Vec::new()];
    let mut depth = 0usize;
    for (i, token) in tokens.iter().enumerate().skip(open) {
        match token {
            Token::Punct('(') => {
                depth += 1;
                if depth == 1 {
                    continue;
                }
            }
            Token::Punct(')') => {
                depth -= 1;
                if depth == 0 {
                    return Some((args, i + 1));
                }
            }
            Token::Punct(',') if depth == 1 => {
                args.push(Vec::new());
                continue;
            }
            _ => {}
        }
        args.last_mut()?.push(token.clone());
    }
    None
}

/// Joins the identifiers either side of each `##` into one.
fn paste(tokens: Vec<Token>) -> Vec<Token> {
    let mut out: Vec<Token> = Vec::with_capacity(tokens.len());
    let mut iter = tokens.into_iter().peekable();
    while let Some(token) = iter.next() {
        if token == Token::Punct('#') && iter.peek() == Some(&Token::Punct('#')) {
            iter.next();
            if let (Some(Token::Ident(left)), Some(Token::Ident(right))) = (out.pop(), iter.next())
            {
                out.push(Token::Ident(left + &right));
            }
            continue;
        }
        out.push(token);
    }
    out
}

/// Joins adjacent string literals, as `"removed " name " dragon hide"` is after expansion.
fn join_strings(tokens: Vec<Token>) -> Vec<Token> {
    let mut out: Vec<Token> = Vec::with_capacity(tokens.len());
    for token in tokens {
        if let (Some(Token::Str(left)), Token::Str(right)) = (out.last_mut(), &token) {
            left.push_str(right);
            continue;
        }
        out.push(token);
    }
    out
}

/// Removes `//` and `/* */` comments, leaving string and character literals alone and keeping
/// line breaks so line numbers stay true.
fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' | '\'' => {
                out.push(c);
                while let Some(d) = chars.next() {
                    out.push(d);
                    if d == '\\' {
                        if let Some(escaped) = chars.next() {
                            out.push(escaped);
                        }
                    } else if d == c || d == '\n' {
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'/') => {
                for d in chars.by_ref() {
                    if d == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut last = ' ';
                for d in chars.by_ref() {
                    if d == '\n' {
                        out.push('\n');
                    }
                    if last == '*' && d == '/' {
                        break;
                    }
                    last = d;
                }
                out.push(' ');
            }
            _ => out.push(c),
        }
    }
    out
}

fn tokenize(file: &str, code: &str) -> Result<Vec<Token>, ExtractError> {
    let mut tokens = Vec::new();
    let mut chars = code.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c.is_ascii_alphabetic() || c == '_' {
            let mut ident = String::new();
            while let Some(&d) = chars
                .peek()
                .filter(|d| d.is_ascii_alphanumeric() || **d == '_')
            {
                ident.push(d);
                chars.next();
            }
            tokens.push(Token::Ident(ident));
        } else if c.is_ascii_digit() {
            let mut number = String::new();
            while let Some(&d) = chars
                .peek()
                .filter(|d| d.is_ascii_alphanumeric() || **d == '.')
            {
                number.push(d);
                chars.next();
            }
            tokens.push(Token::Number(number));
        } else if c == '"' || c == '\'' {
            chars.next();
            let mut text = String::new();
            loop {
                match chars.next() {
                    Some('\\') => match chars.next() {
                        Some('n') => text.push('\n'),
                        Some('t') => text.push('\t'),
                        Some(escaped) => text.push(escaped),
                        None => break,
                    },
                    Some(d) if d == c => break,
                    Some(d) => text.push(d),
                    None => {
                        return Err(ExtractError::Unreadable {
                            file: file.to_owned(),
                            what: "an unterminated literal".to_owned(),
                        });
                    }
                }
            }
            tokens.push(if c == '"' {
                Token::Str(text)
            } else {
                Token::Char(text)
            });
        } else {
            chars.next();
            tokens.push(Token::Punct(c));
        }
    }
    Ok(tokens)
}
