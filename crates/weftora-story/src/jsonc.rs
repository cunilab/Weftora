//! JSONC: JSON plus `//` and `/* */` comments and trailing commas (ADR 0004).
//!
//! Comments and trailing commas are blanked out with spaces (newlines kept), then the text is parsed by
//! `serde_json`, so error line and column numbers still point at the author's original text.

use serde::de::DeserializeOwned;

/// Parses JSONC text into `T`.
pub fn from_str<T: DeserializeOwned>(text: &str) -> serde_json::Result<T> {
    serde_json::from_str(&to_json(text))
}

/// Returns `text` with comments and trailing commas replaced by spaces. Byte offsets and line breaks are
/// preserved. Unterminated block comments are left in place so `serde_json` reports them.
pub fn to_json(text: &str) -> String {
    let mut out: Vec<u8> = text.as_bytes().to_vec();
    let bytes = text.as_bytes();
    let mut i = 0;
    // Position of the last `,` seen outside strings/comments with only whitespace/comments after it.
    let mut pending_comma: Option<usize> = None;
    // Last significant byte outside strings/comments (`"` for a closed string).
    let mut last = b'\0';

    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                pending_comma = None;
                last = b'"';
                i += 1;
                while i < bytes.len() {
                    match bytes[i] {
                        b'\\' => i += 2,
                        b'"' => {
                            i += 1;
                            break;
                        }
                        _ => i += 1,
                    }
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    out[i] = b' ';
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                let Some(end) = find(bytes, i + 2, b"*/") else {
                    break;
                };
                for byte in &mut out[i..end + 2] {
                    if *byte != b'\n' && *byte != b'\r' {
                        *byte = b' ';
                    }
                }
                i = end + 2;
            }
            b',' => {
                // Only a comma that follows a value can be trailing; `"a": ,` stays an error.
                pending_comma = (!matches!(last, b':' | b',' | b'[' | b'{' | b'\0')).then_some(i);
                last = b',';
                i += 1;
            }
            b'}' | b']' => {
                if let Some(comma) = pending_comma.take() {
                    out[comma] = b' ';
                }
                last = bytes[i];
                i += 1;
            }
            b' ' | b'\t' | b'\r' | b'\n' => i += 1,
            other => {
                pending_comma = None;
                last = other;
                i += 1;
            }
        }
    }
    // Only ASCII bytes were replaced with ASCII spaces, so the result is still valid UTF-8.
    String::from_utf8(out).unwrap_or_else(|_| text.to_owned())
}

fn find(haystack: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    haystack
        .get(from..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn parse(text: &str) -> Value {
        from_str(text).unwrap()
    }

    #[test]
    fn plain_json_is_unchanged() {
        let text = r#"{"a": [1, 2], "b": "x"}"#;
        assert_eq!(to_json(text), text);
    }

    #[test]
    fn strips_line_and_block_comments() {
        let text = "{\n  // line\n  \"a\": 1, /* block\n spanning */ \"b\": 2\n}";
        assert_eq!(parse(text), json!({"a": 1, "b": 2}));
        assert_eq!(to_json(text).lines().count(), text.lines().count());
    }

    #[test]
    fn strips_trailing_commas() {
        assert_eq!(
            parse("{\"a\": [1, 2, ], \"b\": {\"c\": 3,}, }"),
            json!({"a": [1, 2], "b": {"c": 3}})
        );
        assert_eq!(parse("[1, // two\n 2, /* end */ ]"), json!([1, 2]));
    }

    #[test]
    fn keeps_comment_markers_and_commas_inside_strings() {
        assert_eq!(
            parse(r#"{"url": "http://x/*y*/", "s": "a,]", "q": "\"//\""}"#),
            json!({"url": "http://x/*y*/", "s": "a,]", "q": "\"//\""})
        );
    }

    #[test]
    fn errors_keep_original_line_numbers() {
        let err = from_str::<Value>("{\n  // comment\n  \"a\": ,\n}").unwrap_err();
        assert_eq!(err.line(), 3);
    }

    #[test]
    fn unterminated_block_comment_is_an_error() {
        assert!(from_str::<Value>("{\"a\": 1 /* oops }").is_err());
    }
}
