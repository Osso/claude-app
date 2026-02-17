use syntect::highlighting::ThemeSet;
use syntect::html::highlighted_html_for_string;
use syntect::parsing::SyntaxSet;

/// Render assistant text with diff highlighting and syntax highlighting for code blocks.
///
/// Returns an HTML string suitable for `dangerous_inner_html` in Dioxus.
pub fn render_assistant_text(text: &str) -> String {
    let parts = split_fenced_blocks(text);
    let mut html = String::new();
    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    let theme = &ts.themes["base16-ocean.dark"];

    for part in parts {
        match part {
            TextPart::Plain(s) => {
                html.push_str("<span>");
                html.push_str(&escape_html(s));
                html.push_str("</span>");
            }
            TextPart::Code { lang, body } => {
                if is_diff_content(body) {
                    html.push_str("<pre class=\"diff-block\">");
                    render_diff_lines(body, &mut html);
                    html.push_str("</pre>");
                } else {
                    render_code_block(&ss, theme, lang, body, &mut html);
                }
            }
        }
    }

    html
}

enum TextPart<'a> {
    Plain(&'a str),
    Code { lang: &'a str, body: &'a str },
}

/// Split text into alternating plain text and fenced code block segments.
fn split_fenced_blocks(text: &str) -> Vec<TextPart<'_>> {
    let mut parts = Vec::new();
    let mut rest = text;

    loop {
        match find_fence_open(rest) {
            None => {
                if !rest.is_empty() {
                    parts.push(TextPart::Plain(rest));
                }
                break;
            }
            Some((pre, lang, after_open)) => {
                if !pre.is_empty() {
                    parts.push(TextPart::Plain(pre));
                }
                match find_fence_close(after_open) {
                    Some((body, after_close)) => {
                        parts.push(TextPart::Code { lang, body });
                        rest = after_close;
                    }
                    None => {
                        // Unclosed fence — treat remainder as code
                        parts.push(TextPart::Code {
                            lang,
                            body: after_open,
                        });
                        break;
                    }
                }
            }
        }
    }

    parts
}

/// Find the next opening fence (``` at start of line, optionally followed by language).
/// Returns (text_before_fence, language, text_after_fence_line).
fn find_fence_open(text: &str) -> Option<(&str, &str, &str)> {
    let mut search_from = 0;
    loop {
        let idx = text[search_from..].find("```")?;
        let abs = search_from + idx;

        // Must be at start of text or preceded by newline
        if abs > 0 && text.as_bytes()[abs - 1] != b'\n' {
            search_from = abs + 3;
            continue;
        }

        let after_backticks = &text[abs + 3..];
        let line_end = after_backticks.find('\n').unwrap_or(after_backticks.len());
        let lang = after_backticks[..line_end].trim();
        let body_start = if line_end < after_backticks.len() {
            abs + 3 + line_end + 1
        } else {
            abs + 3 + line_end
        };

        return Some((&text[..abs], lang, &text[body_start..]));
    }
}

/// Find the closing fence (``` at start of line). Returns (body, text_after_close_line).
fn find_fence_close(text: &str) -> Option<(&str, &str)> {
    let mut search_from = 0;
    loop {
        let idx = text[search_from..].find("```")?;
        let abs = search_from + idx;

        // Must be at start of text or preceded by newline
        if abs > 0 && text.as_bytes()[abs - 1] != b'\n' {
            search_from = abs + 3;
            continue;
        }

        let after = &text[abs + 3..];
        let line_end = after.find('\n').unwrap_or(after.len());
        let rest_start = if line_end < after.len() {
            abs + 3 + line_end + 1
        } else {
            abs + 3 + line_end
        };

        return Some((&text[..abs], &text[rest_start..]));
    }
}

/// Detect whether code block content looks like a diff.
fn is_diff_content(body: &str) -> bool {
    body.lines()
        .any(|line| line.starts_with('+') || line.starts_with('-') || line.starts_with("@@"))
}

/// Render diff lines with colored backgrounds via CSS classes.
fn render_diff_lines(body: &str, html: &mut String) {
    for line in body.lines() {
        let escaped = escape_html(line);
        if line.starts_with("@@") {
            html.push_str("<span class=\"diff-line diff-line-hunk\">");
        } else if line.starts_with('+') && !line.starts_with("+++") {
            html.push_str("<span class=\"diff-line diff-line-add\">");
        } else if line.starts_with('-') && !line.starts_with("---") {
            html.push_str("<span class=\"diff-line diff-line-del\">");
        } else {
            html.push_str("<span class=\"diff-line\">");
        }
        html.push_str(&escaped);
        html.push_str("</span>");
    }
}

/// Render a code block with optional syntax highlighting via syntect.
fn render_code_block(
    ss: &SyntaxSet,
    theme: &syntect::highlighting::Theme,
    lang: &str,
    body: &str,
    html: &mut String,
) {
    if !lang.is_empty() {
        if let Some(syntax) = ss.find_syntax_by_token(lang) {
            if let Ok(highlighted) = highlighted_html_for_string(body, ss, syntax, theme) {
                html.push_str(&highlighted);
                return;
            }
        }
    }
    // Fallback: plain monospace
    html.push_str("<pre class=\"code-block\"><code>");
    html.push_str(&escape_html(body));
    html.push_str("</code></pre>");
}

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_escaped_and_wrapped() {
        let result = render_assistant_text("Hello <world> & \"friends\"");
        assert!(result.contains("&lt;world&gt;"));
        assert!(result.contains("&amp;"));
        assert!(result.contains("&quot;"));
        assert!(result.starts_with("<span>"));
    }

    #[test]
    fn code_block_without_diff() {
        let input = "before\n```rust\nfn main() {}\n```\nafter";
        let result = render_assistant_text(input);
        // Should contain syntect output (has <pre> from syntect) or fallback pre/code
        assert!(result.contains("main"));
        assert!(result.contains("<span>before"));
        assert!(result.contains("<span>after"));
    }

    #[test]
    fn diff_block_detected_and_colored() {
        let input = "```diff\n@@ -1,3 +1,3 @@\n-old line\n+new line\n context\n```";
        let result = render_assistant_text(input);
        assert!(result.contains("diff-line-hunk")); // @@ line
        assert!(result.contains("diff-line-del"));  // - line
        assert!(result.contains("diff-line-add"));  // + line
    }

    #[test]
    fn triple_plus_minus_not_colored_as_diff_lines() {
        let input = "```\n--- a/file.rs\n+++ b/file.rs\n-removed\n+added\n```";
        let result = render_assistant_text(input);
        // --- and +++ lines should NOT get diff coloring
        assert!(result.contains("<span class=\"diff-line\">--- a/file.rs</span>"));
        assert!(result.contains("<span class=\"diff-line\">+++ b/file.rs</span>"));
        // But - and + lines should
        assert!(result.contains("diff-line-del"));
        assert!(result.contains("diff-line-add"));
    }

    #[test]
    fn unclosed_fence_treated_as_code() {
        let input = "text\n```rust\nfn foo() {}";
        let result = render_assistant_text(input);
        assert!(result.contains("foo"));
    }

    #[test]
    fn no_code_blocks() {
        let result = render_assistant_text("just plain text");
        assert_eq!(result, "<span>just plain text</span>");
    }

    #[test]
    fn empty_input() {
        let result = render_assistant_text("");
        assert_eq!(result, "");
    }

    #[test]
    fn multiple_code_blocks() {
        let input = "a\n```\nblock1\n```\nb\n```\nblock2\n```\nc";
        let result = render_assistant_text(input);
        assert!(result.contains("block1"));
        assert!(result.contains("block2"));
        // Three plain segments: a\n, \nb\n, \nc
        let span_count = result.matches("<span>").count();
        assert!(span_count >= 3);
    }
}
