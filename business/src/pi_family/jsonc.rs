//! JSON with comments (JSONC), as OpenCode reads its configuration (`jsonc-parser` with
//! `allowTrailingComma`).
//!
//! `//` line comments, `/* */` block comments and trailing commas before `}` or `]` are blanked
//! out, then the text is parsed as strict JSON. Comments and commas become spaces (block comments
//! keep their line breaks), so error locations still point into the original text. Text inside
//! strings, escapes included, is never touched: `"http://x"` keeps its `//`.

use serde_json::Value;

/// Parses JSONC `text`. Empty input (only whitespace or comments) is `null`, as OpenCode treats an
/// empty file as an empty config. The error is a location-only summary, never file content.
pub fn parse(text: &str) -> Result<Value, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let stripped = strip(text)?;

    if stripped.trim().is_empty() {
        return Ok(Value::Null);
    }

    serde_json::from_str(&stripped)
        .map_err(|error| format!("Invalid JSON at line {}, column {}", error.line(), error.column()))
}

/// `text` with comments and trailing commas replaced by spaces; an unterminated block comment is
/// an error naming where it starts.
pub fn strip(text: &str) -> Result<String, String> {
    let without_comments = strip_comments(text)?;
    Ok(strip_trailing_commas(&without_comments))
}

/// Whether `text` has a comment outside strings (an unterminated one included): what a rewrite as
/// plain JSON would lose.
pub fn has_comments(text: &str) -> bool {
    match strip_comments(text) {
        Ok(stripped) => stripped != text,
        Err(_) => true,
    }
}

fn strip_comments(text: &str) -> Result<String, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    let mut index = 0;

    while index < chars.len() {
        let current = chars[index];
        let next = chars.get(index + 1).copied();

        if in_string {
            out.push(current);

            if current == '\\' {
                if let Some(escaped) = next {
                    out.push(escaped);
                    index += 1;
                }
            } else if current == '"' {
                in_string = false;
            }

            index += 1;
            continue;
        }

        match (current, next) {
            ('"', _) => {
                in_string = true;
                out.push(current);
                index += 1;
            }

            ('/', Some('/')) => {
                while index < chars.len() && !matches!(chars[index], '\n' | '\r') {
                    out.push(' ');
                    index += 1;
                }
            }

            ('/', Some('*')) => {
                let start = index;
                let end = (index + 2..chars.len().saturating_sub(1))
                    .find(|&at| chars[at] == '*' && chars[at + 1] == '/')
                    .ok_or_else(|| {
                        let (line, column) = location(&chars, start);
                        format!("Unterminated comment at line {line}, column {column}")
                    })?;

                for &skipped in &chars[start..end + 2] {
                    out.push(if matches!(skipped, '\n' | '\r') { skipped } else { ' ' });
                }
                index = end + 2;
            }

            _ => {
                out.push(current);
                index += 1;
            }
        }
    }

    Ok(out)
}

/// Blanks every comma (outside strings) whose next non-whitespace character closes an object or
/// an array. Runs after [`strip_comments`], so only whitespace can separate the two.
fn strip_trailing_commas(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    let mut escaped = false;

    for (index, &current) in chars.iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if current == '\\' {
                escaped = true;
            } else if current == '"' {
                in_string = false;
            }

            out.push(current);
            continue;
        }

        if current == '"' {
            in_string = true;
        }

        let closes = || {
            chars[index + 1..]
                .iter()
                .find(|next| !next.is_whitespace())
                .is_some_and(|next| matches!(next, '}' | ']'))
        };

        if current == ',' && closes() {
            out.push(' ');
        } else {
            out.push(current);
        }
    }

    out
}

/// One-based line and column of `chars[index]` (`\r\n` counts as one line break).
fn location(chars: &[char], index: usize) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;

    for (at, &current) in chars[..index].iter().enumerate() {
        let crlf = current == '\r' && chars.get(at + 1) == Some(&'\n');

        if current == '\n' || (current == '\r' && !crlf) {
            line += 1;
            column = 1;
        } else if !crlf {
            column += 1;
        }
    }

    (line, column)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn comments_and_trailing_commas_are_ignored() {
        let text = r#"{
  // the model
  "model": "anthropic/claude", /* inline */
  "list": [1, 2, /* last */ ],
  "nested": { "a": { "b": true, }, },
}"#;

        assert_eq!(
            parse(text),
            Ok(json!({ "model": "anthropic/claude", "list": [1, 2], "nested": { "a": { "b": true } } }))
        );
    }

    #[test]
    fn comment_markers_inside_strings_are_text() {
        let text = r#"{ "url": "http://x/*y*/", "quote": "a \"// b\" c\\", "comma": ",}" }"#;

        assert_eq!(
            parse(text),
            Ok(json!({ "url": "http://x/*y*/", "quote": "a \"// b\" c\\", "comma": ",}" }))
        );
    }

    #[test]
    fn crlf_line_endings_and_a_byte_order_mark_are_accepted() {
        let text = "\u{feff}{\r\n  // note\r\n  \"a\": 1,\r\n  /* multi\r\n line */\r\n}\r\n";

        assert_eq!(parse(text), Ok(json!({ "a": 1 })));
    }

    #[test]
    fn an_unterminated_block_comment_is_an_error_with_its_location() {
        assert_eq!(
            parse("{\r\n  \"a\": 1 /* never closed\r\n}"),
            Err("Unterminated comment at line 2, column 10".to_owned())
        );
        assert_eq!(parse("/*/"), Err("Unterminated comment at line 1, column 1".to_owned()));
    }

    #[test]
    fn errors_keep_the_original_location_and_no_content() {
        let error = parse("{\n  /* a\n  b */ \"token\": secret\n}").unwrap_err();

        assert_eq!(error, "Invalid JSON at line 3, column 17");
    }

    #[test]
    fn empty_or_comment_only_text_is_null() {
        assert_eq!(parse(""), Ok(Value::Null));
        assert_eq!(parse("  // nothing here\n"), Ok(Value::Null));
    }

    #[test]
    fn comments_are_detected_outside_strings_only() {
        assert!(has_comments("{ // a\n}"));
        assert!(has_comments("{ /* a */ }"));
        assert!(has_comments("{ \"a\": 1 /* never closed"));
        assert!(!has_comments(r#"{ "url": "http://x/*y*/", "q": "a \"// b\"" }"#));
        assert!(
            !has_comments(r#"{ "a": [1, 2,], }"#),
            "trailing commas are not comments"
        );
    }

    #[test]
    fn a_comma_before_another_value_is_kept() {
        assert_eq!(parse("[1, // one\n 2]"), Ok(json!([1, 2])));
        assert!(parse("[1,,]").is_err(), "only one trailing comma is dropped");
    }
}
