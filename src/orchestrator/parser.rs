/// All recognized output prefixes
const ALL_PREFIXES: &[&str] = &[
    "CREW:", "RELIEVE:", "TASK:", "APPROVED:", "REJECTED:",
    "COMPLETE:", "BLOCKED:", "INTERRUPT:", "EVALUATION:", "OBSERVATION:",
];

/// Prefixes whose content is only the remainder of the same line
const SINGLE_LINE_PREFIXES: &[&str] = &["CREW:", "RELIEVE:"];

/// Strip leading markdown bold markers (e.g. "**TASK:" -> "TASK:")
fn strip_markdown_bold(line: &str) -> &str {
    line.strip_prefix("**").unwrap_or(line)
}

/// Check if a line starts with a recognized prefix (ignoring markdown bold)
fn recognized_prefix(line: &str) -> Option<&'static str> {
    let clean = strip_markdown_bold(line);
    ALL_PREFIXES.iter().find(|&&p| clean.starts_with(p)).copied()
}

/// Extract structured sections from multi-line agent output.
/// Returns (prefix, content) pairs where content includes continuation lines.
pub fn extract_sections(text: &str) -> Vec<(&'static str, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut sections = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim_start();
        if let Some(prefix) = recognized_prefix(line) {
            let clean = strip_markdown_bold(line);
            let first = clean[prefix.len()..].trim().trim_end_matches("**");

            if SINGLE_LINE_PREFIXES.contains(&prefix) {
                sections.push((prefix, first.to_string()));
                i += 1;
                continue;
            }

            // Multi-line: collect until next prefix or end
            let mut content = first.to_string();
            i += 1;
            while i < lines.len() {
                if recognized_prefix(lines[i].trim_start()).is_some() {
                    break;
                }
                content.push('\n');
                content.push_str(lines[i]);
                i += 1;
            }

            sections.push((prefix, content.trim_end().to_string()));
        } else {
            i += 1;
        }
    }

    sections
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line_crew() {
        let sections = extract_sections("CREW: 2");
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].0, "CREW:");
        assert_eq!(sections[0].1, "2");
    }

    #[test]
    fn multi_line_task() {
        let text = "TASK: Implement feature\nDetails about the feature\nMore details";
        let sections = extract_sections(text);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].0, "TASK:");
        assert!(sections[0].1.contains("Implement feature"));
        assert!(sections[0].1.contains("More details"));
    }

    #[test]
    fn bold_prefix_stripped() {
        let sections = extract_sections("**TASK:** Implement feature");
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].0, "TASK:");
    }

    #[test]
    fn multiple_sections() {
        let text = "CREW: 2\nTASK: Do something\nWith details\nAPPROVED: developer-0 looks good";
        let sections = extract_sections(text);
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0].0, "CREW:");
        assert_eq!(sections[1].0, "TASK:");
        assert_eq!(sections[2].0, "APPROVED:");
    }
}
