/// Simple line-based Markdown analyzer
/// This analyzes individual lines to determine their Markdown element type
/// without relying on full AST parsing, avoiding sync issues
/// Table column alignment derived from separator row
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TableAlignment {
    Left,   // :---
    Center, // :---:
    Right,  // ---:
    None,   // ---
}

#[derive(Debug, Clone, PartialEq)]
pub enum LineType {
    Heading(usize), // Level 1-6
    ListItem,
    OrderedListItem,
    TaskListItem(bool), // checked
    Blockquote,
    CodeFence(Option<String>), // language
    InCode,                    // Inside code block
    HorizontalRule,
    Image(String, String),               // (alt_text, path)
    TableHeader(Vec<String>),            // Table header row with cell contents
    TableSeparator(Vec<TableAlignment>), // Table separator row with alignments
    TableRow(Vec<String>),               // Table data row with cell contents
    FrontMatterDelimiter,                // --- or +++ at start/end of front matter
    FrontMatterContent,                  // YAML/TOML content inside front matter
    Text,
}

pub struct LineAnalyzer;

impl LineAnalyzer {
    /// Analyze a line and determine its Markdown type
    pub fn analyze_line(line: &str) -> LineType {
        let trimmed = line.trim_start();

        // Front matter delimiter (--- or +++)
        if (trimmed == "---" || trimmed == "+++") && line == trimmed {
            return LineType::FrontMatterDelimiter;
        }

        // Heading
        if let Some(rest) = trimmed.strip_prefix('#') {
            let mut level = 1;
            let mut chars = rest.chars();
            while let Some('#') = chars.next() {
                level += 1;
                if level > 6 {
                    break;
                }
            }
            if level <= 6
                && rest
                    .chars()
                    .nth(level - 1)
                    .is_some_and(|c| c.is_whitespace())
            {
                return LineType::Heading(level);
            }
        }

        // Horizontal rule
        if trimmed.starts_with("---")
            || trimmed.starts_with("***")
            || trimmed.starts_with("___")
                && trimmed
                    .chars()
                    .all(|c| c == '-' || c == '*' || c == '_' || c.is_whitespace())
        {
            return LineType::HorizontalRule;
        }

        // Code fence
        if trimmed.starts_with("```") {
            let lang = trimmed
                .strip_prefix("```")
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
            return LineType::CodeFence(lang);
        }

        // Task list item
        if trimmed.starts_with("- [") {
            if trimmed.contains("- [x]") || trimmed.contains("- [X]") {
                return LineType::TaskListItem(true);
            } else if trimmed.contains("- [ ]") {
                return LineType::TaskListItem(false);
            }
        }

        // Unordered list
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
            return LineType::ListItem;
        }

        // Ordered list
        if let Some(ch) = trimmed.chars().next()
            && ch.is_ascii_digit()
        {
            let rest = &trimmed[1..];
            if rest.starts_with(". ") || rest.starts_with(") ") {
                return LineType::OrderedListItem;
            }
        }

        // Blockquote
        if trimmed.starts_with("> ") {
            return LineType::Blockquote;
        }

        // Image: ![alt text](path)
        if trimmed.starts_with("![")
            && let Some(alt_end) = trimmed.find("](")
        {
            let alt_text = &trimmed[2..alt_end];
            let rest = &trimmed[alt_end + 2..];
            if let Some(path_end) = rest.find(')') {
                let path = &rest[..path_end];
                return LineType::Image(alt_text.to_string(), path.to_string());
            }
        }

        LineType::Text
    }

    /// Check if a line contains bold text
    pub fn contains_bold(line: &str) -> bool {
        line.contains("**") || line.contains("__")
    }

    /// Check if a line contains italic text
    pub fn contains_italic(line: &str) -> bool {
        line.contains('*') || line.contains('_')
    }

    /// Check if a line contains strikethrough
    pub fn contains_strikethrough(line: &str) -> bool {
        line.contains("~~")
    }

    /// Check if a line contains inline code
    pub fn contains_inline_code(line: &str) -> bool {
        line.contains('`') && !line.trim().starts_with("```")
    }

    /// Check if a line contains a link
    pub fn contains_link(line: &str) -> bool {
        line.contains('[') && line.contains("](")
    }

    /// Check if a line looks like a table row (contains pipe delimiters)
    pub fn is_table_row(line: &str) -> bool {
        let trimmed = line.trim();
        trimmed.contains('|') && !trimmed.starts_with("```")
    }

    /// Check if a line is a table separator row (e.g., |---|:---:|---:|)
    pub fn is_table_separator(line: &str) -> bool {
        let trimmed = line.trim();
        if !trimmed.contains('|') {
            return false;
        }
        // Remove pipes and check if remaining content is only dashes, colons, spaces
        let content: String = trimmed
            .chars()
            .filter(|c| *c != '|' && !c.is_whitespace())
            .collect();
        !content.is_empty() && content.chars().all(|c| c == '-' || c == ':')
    }

    /// Parse table cell contents from a row
    pub fn parse_table_cells(line: &str) -> Vec<String> {
        let trimmed = line.trim();
        let stripped = trimmed.trim_matches('|');
        stripped
            .split('|')
            .map(|cell| cell.trim().to_string())
            .collect()
    }

    /// Parse alignment from separator row
    pub fn parse_table_alignment(line: &str) -> Vec<TableAlignment> {
        Self::parse_table_cells(line)
            .iter()
            .map(|cell| {
                let cell = cell.trim();
                let starts_colon = cell.starts_with(':');
                let ends_colon = cell.ends_with(':');
                match (starts_colon, ends_colon) {
                    (true, true) => TableAlignment::Center,
                    (true, false) => TableAlignment::Left,
                    (false, true) => TableAlignment::Right,
                    (false, false) => TableAlignment::None,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heading_detection() {
        assert_eq!(
            LineAnalyzer::analyze_line("# Heading 1"),
            LineType::Heading(1)
        );
        assert_eq!(
            LineAnalyzer::analyze_line("## Heading 2"),
            LineType::Heading(2)
        );
        assert_eq!(
            LineAnalyzer::analyze_line("### Heading 3"),
            LineType::Heading(3)
        );
    }

    #[test]
    fn test_list_detection() {
        assert_eq!(LineAnalyzer::analyze_line("- Item"), LineType::ListItem);
        assert_eq!(LineAnalyzer::analyze_line("* Item"), LineType::ListItem);
        assert_eq!(
            LineAnalyzer::analyze_line("1. Item"),
            LineType::OrderedListItem
        );
    }

    #[test]
    fn test_task_list() {
        assert_eq!(
            LineAnalyzer::analyze_line("- [ ] Todo"),
            LineType::TaskListItem(false)
        );
        assert_eq!(
            LineAnalyzer::analyze_line("- [x] Done"),
            LineType::TaskListItem(true)
        );
    }

    #[test]
    fn test_blockquote() {
        assert_eq!(LineAnalyzer::analyze_line("> Quote"), LineType::Blockquote);
    }

    #[test]
    fn test_code_fence() {
        assert_eq!(
            LineAnalyzer::analyze_line("```rust"),
            LineType::CodeFence(Some("rust".to_string()))
        );
        assert_eq!(LineAnalyzer::analyze_line("```"), LineType::CodeFence(None));
    }

    #[test]
    fn test_is_table_row() {
        assert!(LineAnalyzer::is_table_row("| Name | Age |"));
        assert!(LineAnalyzer::is_table_row("|Name|Age|"));
        assert!(LineAnalyzer::is_table_row("| Name | Age | City |"));
        assert!(!LineAnalyzer::is_table_row("Normal text"));
        // Note: "Text with | pipe" is detected as table row, but context validation
        // (requiring separator row) ensures it's not treated as a valid table
        assert!(!LineAnalyzer::is_table_row("```|code|```"));
    }

    #[test]
    fn test_is_table_separator() {
        assert!(LineAnalyzer::is_table_separator("|---|---|"));
        assert!(LineAnalyzer::is_table_separator("| --- | --- |"));
        assert!(LineAnalyzer::is_table_separator("| :--- | ---: |"));
        assert!(LineAnalyzer::is_table_separator("|:---:|:---:|"));
        assert!(LineAnalyzer::is_table_separator("| :--- | :---: | ---: |"));
        assert!(!LineAnalyzer::is_table_separator("| Name | Age |"));
        assert!(!LineAnalyzer::is_table_separator("Normal text"));
    }

    #[test]
    fn test_parse_table_cells() {
        let cells = LineAnalyzer::parse_table_cells("| Name | Age |");
        assert_eq!(cells, vec!["Name", "Age"]);

        let cells = LineAnalyzer::parse_table_cells("|Name|Age|");
        assert_eq!(cells, vec!["Name", "Age"]);

        let cells = LineAnalyzer::parse_table_cells("| Name | Age | City |");
        assert_eq!(cells, vec!["Name", "Age", "City"]);

        let cells = LineAnalyzer::parse_table_cells("|  Spaced  |  Content  |");
        assert_eq!(cells, vec!["Spaced", "Content"]);
    }

    #[test]
    fn test_parse_table_alignment() {
        let alignments = LineAnalyzer::parse_table_alignment("| :--- | ---: | :---: | --- |");
        assert_eq!(
            alignments,
            vec![
                TableAlignment::Left,
                TableAlignment::Right,
                TableAlignment::Center,
                TableAlignment::None,
            ]
        );

        let alignments = LineAnalyzer::parse_table_alignment("|:---|---:|:---:|---|");
        assert_eq!(
            alignments,
            vec![
                TableAlignment::Left,
                TableAlignment::Right,
                TableAlignment::Center,
                TableAlignment::None,
            ]
        );
    }

    #[test]
    fn test_front_matter_delimiter() {
        assert_eq!(
            LineAnalyzer::analyze_line("---"),
            LineType::FrontMatterDelimiter
        );
        assert_eq!(
            LineAnalyzer::analyze_line("+++"),
            LineType::FrontMatterDelimiter
        );
        // With spaces should not be treated as front matter delimiter
        assert_eq!(LineAnalyzer::analyze_line("--- "), LineType::HorizontalRule);
        // Not at start should be horizontal rule
        assert_eq!(LineAnalyzer::analyze_line(" ---"), LineType::HorizontalRule);
    }
}
