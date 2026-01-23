//! URL detection and extraction utilities.
//!
//! This module provides utilities for detecting and extracting URLs from text,
//! including handling of ANSI escape sequences commonly found in terminal output.

use once_cell::sync::Lazy;
use regex::Regex;

/// Regex pattern for detecting URLs.
/// Matches http:// and https:// URLs, stopping at whitespace and common delimiters.
/// Allows `()[]` characters to support URLs like Wikipedia (e.g., Function_(mathematics)).
static URL_PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(r#"https?://[^\s<>{}"'`]+"#).unwrap());

/// ANSI escape sequence pattern for stripping terminal formatting.
/// Matches both CSI sequences (e.g., color codes) and OSC sequences (e.g., hyperlinks).
static ANSI_ESCAPE_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]|\x1b\].*?\x07").unwrap());

/// Strip ANSI escape codes from text.
///
/// This removes all ANSI formatting sequences, leaving only the visible text.
/// This is important for URL detection as ANSI codes affect byte positions but
/// not visible character positions.
pub fn strip_ansi(text: &str) -> String {
    ANSI_ESCAPE_PATTERN.replace_all(text, "").to_string()
}

/// Extract the URL at a given character position in a line.
///
/// This function first strips ANSI escape codes from the line, then checks if
/// the given character position falls within any URL. If so, the cleaned URL
/// is returned.
///
/// # Arguments
/// * `line` - The line of text (may contain ANSI escape codes)
/// * `char_pos` - The character position (0-indexed, after ANSI stripping)
///
/// # Returns
/// * `Some(url)` if the position falls within a URL
/// * `None` if no URL at that position
pub fn extract_url_at_position(line: &str, char_pos: usize) -> Option<String> {
    let stripped = strip_ansi(line);

    for mat in URL_PATTERN.find_iter(&stripped) {
        // Convert byte positions to character positions
        let start_chars = stripped[..mat.start()].chars().count();
        let end_chars = stripped[..mat.end()].chars().count();

        if char_pos >= start_chars && char_pos < end_chars {
            return Some(clean_url_trailing(mat.as_str()));
        }
    }
    None
}

/// Remove common trailing punctuation from URLs.
///
/// URLs in text often have trailing punctuation that's part of the sentence
/// but not the URL itself. This function removes common trailing characters
/// like commas, periods, etc.
///
/// For parentheses and brackets, it uses balanced matching: only removes
/// trailing `)` if there are more `)` than `(` in the URL. This correctly
/// handles URLs like `https://en.wikipedia.org/wiki/Function_(mathematics)`.
fn clean_url_trailing(url: &str) -> String {
    let mut url = url.to_string();

    while let Some(last_char) = url.chars().last() {
        match last_char {
            // Always remove these trailing punctuation marks
            ',' | '.' | ';' | ':' | '\'' | '"' => {
                url.pop();
            }
            // Only remove trailing ')' if unbalanced
            ')' => {
                let open = url.chars().filter(|&c| c == '(').count();
                let close = url.chars().filter(|&c| c == ')').count();
                if close > open {
                    url.pop();
                } else {
                    break;
                }
            }
            // Only remove trailing ']' if unbalanced
            ']' => {
                let open = url.chars().filter(|&c| c == '[').count();
                let close = url.chars().filter(|&c| c == ']').count();
                if close > open {
                    url.pop();
                } else {
                    break;
                }
            }
            _ => break,
        }
    }

    url
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== strip_ansi tests ==========

    #[test]
    fn test_strip_ansi_no_codes() {
        let text = "Hello, world!";
        assert_eq!(strip_ansi(text), "Hello, world!");
    }

    #[test]
    fn test_strip_ansi_color_codes() {
        // 38;5;32m is a 256-color code (green)
        let text = "\x1b[38;5;32mGreen text\x1b[0m normal";
        assert_eq!(strip_ansi(text), "Green text normal");
    }

    #[test]
    fn test_strip_ansi_bold_and_reset() {
        let text = "\x1b[1mBold\x1b[0m text";
        assert_eq!(strip_ansi(text), "Bold text");
    }

    #[test]
    fn test_strip_ansi_multiple_codes() {
        let text = "\x1b[31m\x1b[1mRed bold\x1b[0m\x1b[0m";
        assert_eq!(strip_ansi(text), "Red bold");
    }

    // ========== clean_url_trailing tests ==========

    #[test]
    fn test_clean_url_trailing_no_punctuation() {
        assert_eq!(
            clean_url_trailing("https://example.com"),
            "https://example.com"
        );
    }

    #[test]
    fn test_clean_url_trailing_period() {
        assert_eq!(
            clean_url_trailing("https://example.com."),
            "https://example.com"
        );
    }

    #[test]
    fn test_clean_url_trailing_comma() {
        assert_eq!(
            clean_url_trailing("https://example.com,"),
            "https://example.com"
        );
    }

    #[test]
    fn test_clean_url_trailing_parenthesis() {
        assert_eq!(
            clean_url_trailing("https://example.com)"),
            "https://example.com"
        );
    }

    #[test]
    fn test_clean_url_trailing_bracket() {
        assert_eq!(
            clean_url_trailing("https://example.com]"),
            "https://example.com"
        );
    }

    #[test]
    fn test_clean_url_trailing_multiple() {
        assert_eq!(
            clean_url_trailing("https://example.com)."),
            "https://example.com"
        );
    }

    #[test]
    fn test_clean_url_trailing_preserves_path() {
        // Should not remove valid URL characters
        assert_eq!(
            clean_url_trailing("https://example.com/path"),
            "https://example.com/path"
        );
    }

    #[test]
    fn test_clean_url_trailing_balanced_parentheses() {
        // Wikipedia-style URLs with balanced parentheses should be preserved
        assert_eq!(
            clean_url_trailing("https://en.wikipedia.org/wiki/Function_(mathematics)"),
            "https://en.wikipedia.org/wiki/Function_(mathematics)"
        );
    }

    #[test]
    fn test_clean_url_trailing_unbalanced_parentheses() {
        // Unbalanced trailing parenthesis should be removed (e.g., "(see https://example.com)")
        assert_eq!(
            clean_url_trailing("https://example.com)"),
            "https://example.com"
        );
    }

    #[test]
    fn test_clean_url_trailing_balanced_brackets() {
        // Balanced brackets should be preserved
        assert_eq!(
            clean_url_trailing("https://example.com/path[1]"),
            "https://example.com/path[1]"
        );
    }

    #[test]
    fn test_clean_url_trailing_unbalanced_brackets() {
        // Unbalanced trailing bracket should be removed
        assert_eq!(
            clean_url_trailing("https://example.com]"),
            "https://example.com"
        );
    }

    #[test]
    fn test_clean_url_trailing_nested_parentheses() {
        // Multiple levels of parentheses
        assert_eq!(
            clean_url_trailing("https://example.com/page_(section_(sub))"),
            "https://example.com/page_(section_(sub))"
        );
    }

    // ========== extract_url_at_position tests ==========

    #[test]
    fn test_extract_url_simple() {
        let line = "Check out https://example.com for more";
        // "Check out " = 10 chars, URL starts at position 10
        assert_eq!(
            extract_url_at_position(line, 10),
            Some("https://example.com".to_string())
        );
        assert_eq!(
            extract_url_at_position(line, 15),
            Some("https://example.com".to_string())
        );
        assert_eq!(
            extract_url_at_position(line, 28),
            Some("https://example.com".to_string())
        );
    }

    #[test]
    fn test_extract_url_at_start() {
        let line = "https://example.com is the URL";
        assert_eq!(
            extract_url_at_position(line, 0),
            Some("https://example.com".to_string())
        );
    }

    #[test]
    fn test_extract_url_no_url() {
        let line = "No URL in this line";
        assert_eq!(extract_url_at_position(line, 5), None);
    }

    #[test]
    fn test_extract_url_before_url() {
        let line = "Check https://example.com here";
        // Position 3 is "c" in "Check", not in the URL
        assert_eq!(extract_url_at_position(line, 3), None);
    }

    #[test]
    fn test_extract_url_after_url() {
        let line = "See https://example.com here";
        // "See https://example.com " = 24 chars, "here" starts at 24
        assert_eq!(extract_url_at_position(line, 25), None);
    }

    #[test]
    fn test_extract_url_with_ansi() {
        // URL wrapped in green color codes
        let line = "Check \x1b[38;5;32mhttps://example.com\x1b[0m here";
        // After stripping: "Check https://example.com here"
        // "Check " = 6 chars, URL starts at position 6
        assert_eq!(
            extract_url_at_position(line, 6),
            Some("https://example.com".to_string())
        );
        assert_eq!(
            extract_url_at_position(line, 15),
            Some("https://example.com".to_string())
        );
    }

    #[test]
    fn test_extract_url_with_trailing_punctuation() {
        let line = "See https://example.com, and more";
        // URL with trailing comma should be cleaned
        assert_eq!(
            extract_url_at_position(line, 10),
            Some("https://example.com".to_string())
        );
    }

    #[test]
    fn test_extract_url_http() {
        let line = "Visit http://example.com now";
        assert_eq!(
            extract_url_at_position(line, 7),
            Some("http://example.com".to_string())
        );
    }

    #[test]
    fn test_extract_url_with_path() {
        let line = "Check https://example.com/path/to/page here";
        assert_eq!(
            extract_url_at_position(line, 10),
            Some("https://example.com/path/to/page".to_string())
        );
    }

    #[test]
    fn test_extract_url_with_query_params() {
        let line = "Go to https://example.com?foo=bar&baz=qux now";
        assert_eq!(
            extract_url_at_position(line, 10),
            Some("https://example.com?foo=bar&baz=qux".to_string())
        );
    }

    #[test]
    fn test_extract_url_multiple_urls() {
        let line = "First: https://first.com Second: https://second.com";
        // First URL starts at 7, second at 33
        assert_eq!(
            extract_url_at_position(line, 10),
            Some("https://first.com".to_string())
        );
        assert_eq!(
            extract_url_at_position(line, 40),
            Some("https://second.com".to_string())
        );
    }

    #[test]
    fn test_extract_url_github_pr() {
        let line = "PR: https://github.com/user/repo/pull/123";
        assert_eq!(
            extract_url_at_position(line, 10),
            Some("https://github.com/user/repo/pull/123".to_string())
        );
    }

    #[test]
    fn test_extract_url_wikipedia() {
        // Wikipedia URLs with parentheses in the path
        let line = "See https://en.wikipedia.org/wiki/Function_(mathematics) for details";
        assert_eq!(
            extract_url_at_position(line, 10),
            Some("https://en.wikipedia.org/wiki/Function_(mathematics)".to_string())
        );
    }

    #[test]
    fn test_extract_url_in_parentheses() {
        // URL wrapped in sentence parentheses - the outer ) should be removed
        let line = "(see https://example.com)";
        assert_eq!(
            extract_url_at_position(line, 10),
            Some("https://example.com".to_string())
        );
    }

    #[test]
    fn test_extract_url_with_fragment() {
        let line = "See https://example.com/page#section for info";
        assert_eq!(
            extract_url_at_position(line, 10),
            Some("https://example.com/page#section".to_string())
        );
    }

    #[test]
    fn test_extract_url_empty_line() {
        let line = "";
        assert_eq!(extract_url_at_position(line, 0), None);
    }

    #[test]
    fn test_extract_url_position_at_end_boundary() {
        let line = "https://example.com";
        // URL is 19 chars (positions 0-18), position 19 is past the end
        assert_eq!(
            extract_url_at_position(line, 18),
            Some("https://example.com".to_string())
        );
        assert_eq!(extract_url_at_position(line, 19), None);
    }

    #[test]
    fn test_extract_url_utf8_before() {
        // Japanese text before URL
        let line = "リンク: https://example.com";
        // "リンク: " = 5 chars (4 Japanese + colon + space = actually "リンク: " is 5 chars)
        // Wait, let me count: リ, ン, ク, :, space = 5 chars
        // No wait: "リンク: " with space after colon
        // リ(1) ン(2) ク(3) :(4)  (5) = 5 chars
        // URL starts at position 5
        assert_eq!(
            extract_url_at_position(line, 5),
            Some("https://example.com".to_string())
        );
    }
}
