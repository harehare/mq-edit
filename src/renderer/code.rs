use std::collections::HashMap;

use lsp_types::SemanticTokens;
use ratatui::{
    style::{Color, Style},
    text::Span,
};
use syntect::{
    easy::HighlightLines,
    highlighting::{Theme, ThemeSet},
    parsing::{SyntaxDefinition, SyntaxReference, SyntaxSet, SyntaxSetBuilder},
};

use super::Renderer;
use crate::document::DocumentBuffer;

/// Semantic token information from LSP
#[derive(Debug, Clone)]
pub struct SemanticToken {
    pub start: usize,    // Character offset in line
    pub length: usize,   // Token length in characters
    pub token_type: u32, // Token type from LSP
    pub modifiers: u32,  // Token modifiers
}

/// Convert LSP SemanticTokens to line-based tokens
pub fn decode_semantic_tokens(
    tokens: &SemanticTokens,
    _content: &str,
) -> HashMap<usize, Vec<SemanticToken>> {
    let mut result: HashMap<usize, Vec<SemanticToken>> = HashMap::new();

    let data = &tokens.data;
    let mut current_line = 0;
    let mut current_start = 0;

    for lsp_token in data {
        let delta_line = lsp_token.delta_line;
        let delta_start = lsp_token.delta_start;
        let length = lsp_token.length;
        let token_type = lsp_token.token_type;
        let token_modifiers_bitset = lsp_token.token_modifiers_bitset;

        // Update position
        if delta_line > 0 {
            current_line += delta_line as usize;
            current_start = delta_start as usize;
        } else {
            current_start += delta_start as usize;
        }

        // Create our token structure
        let token = SemanticToken {
            start: current_start,
            length: length as usize,
            token_type,
            modifiers: token_modifiers_bitset,
        };

        result.entry(current_line).or_default().push(token);
    }

    result
}

/// Code renderer with syntax highlighting
///
/// Supports two modes:
/// 1. Syntect (default) - Static syntax highlighting
/// 2. LSP Semantic Tokens (optional) - Semantic highlighting from language servers
pub struct CodeRenderer {
    /// Default syntect syntax set for static highlighting
    default_syntax_set: SyntaxSet,
    /// Custom syntax set with mq language
    mq_syntax_set: SyntaxSet,
    /// Theme for syntax highlighting
    theme: Theme,
    /// Theme set for loading themes
    theme_set: ThemeSet,
    /// LSP semantic tokens cache (line_idx -> tokens)
    semantic_tokens: HashMap<usize, Vec<SemanticToken>>,
    /// Whether to use semantic tokens for highlighting
    use_semantic_tokens: bool,
}

/// Embedded mq language syntax definition (sublime-syntax format)
const MQ_SUBLIME_SYNTAX: &str = include_str!("../../mq.sublime-syntax");

impl CodeRenderer {
    pub fn new() -> Self {
        Self::with_theme("base16-ocean.dark")
    }

    /// Create a new code renderer with specified theme
    pub fn with_theme(theme_name: &str) -> Self {
        let default_syntax_set = SyntaxSet::load_defaults_newlines();
        let mq_syntax_set = Self::build_mq_syntax_set();
        let theme_set = ThemeSet::load_defaults();
        let theme = theme_set
            .themes
            .get(theme_name)
            .or_else(|| theme_set.themes.get("base16-ocean.dark"))
            .or_else(|| theme_set.themes.values().next())
            .cloned()
            .unwrap_or_default();

        Self {
            default_syntax_set,
            mq_syntax_set,
            theme,
            theme_set,
            semantic_tokens: HashMap::new(),
            use_semantic_tokens: false,
        }
    }

    /// Set the theme by name
    pub fn set_theme(&mut self, theme_name: &str) {
        if let Some(theme) = self.theme_set.themes.get(theme_name) {
            self.theme = theme.clone();
        }
    }

    /// Get list of available theme names
    pub fn available_themes() -> Vec<String> {
        let theme_set = ThemeSet::load_defaults();
        let mut themes: Vec<String> = theme_set.themes.keys().cloned().collect();
        themes.sort();
        themes
    }

    /// Build syntax set with mq language
    fn build_mq_syntax_set() -> SyntaxSet {
        let mut builder = SyntaxSetBuilder::new();
        builder.add_plain_text_syntax();

        if let Ok(mq_syntax) = SyntaxDefinition::load_from_str(
            MQ_SUBLIME_SYNTAX,
            true, // lines_include_newline
            Some("mq"),
        ) {
            builder.add(mq_syntax);
        }

        builder.build()
    }

    /// Update semantic tokens from LSP
    pub fn set_semantic_tokens(&mut self, tokens: HashMap<usize, Vec<SemanticToken>>) {
        self.semantic_tokens = tokens;
    }

    /// Clear semantic tokens
    pub fn clear_semantic_tokens(&mut self) {
        self.semantic_tokens.clear();
    }

    /// Set whether to use semantic tokens for highlighting
    pub fn set_use_semantic_tokens(&mut self, use_semantic_tokens: bool) {
        self.use_semantic_tokens = use_semantic_tokens;
    }

    /// Get syntax reference and corresponding syntax set for a language
    fn get_syntax(&self, language: &str) -> Option<(&SyntaxReference, &SyntaxSet)> {
        if let Some(syntax) = self.mq_syntax_set.find_syntax_by_token(language) {
            return Some((syntax, &self.mq_syntax_set));
        }

        self.default_syntax_set
            .find_syntax_by_token(language)
            .map(|s| (s, &self.default_syntax_set))
    }

    /// Render line with LSP semantic tokens
    fn render_with_semantic_tokens(
        &self,
        content: &str,
        tokens: &[SemanticToken],
    ) -> Vec<Span<'_>> {
        if tokens.is_empty() {
            return vec![Span::raw(content.to_string())];
        }

        let mut spans = Vec::new();
        let mut last_end = 0;

        for token in tokens {
            // Add unstyled text before token
            if token.start > last_end {
                let text = content
                    .chars()
                    .skip(last_end)
                    .take(token.start - last_end)
                    .collect::<String>();
                spans.push(Span::raw(text));
            }

            // Add styled token
            let text = content
                .chars()
                .skip(token.start)
                .take(token.length)
                .collect::<String>();
            let style = self.semantic_token_style(token.token_type, token.modifiers);
            spans.push(Span::styled(text, style));

            last_end = token.start + token.length;
        }

        // Add remaining text
        if last_end < content.chars().count() {
            let text = content.chars().skip(last_end).collect::<String>();
            spans.push(Span::raw(text));
        }

        spans
    }

    /// Get style for semantic token type
    fn semantic_token_style(&self, token_type: u32, _modifiers: u32) -> Style {
        // Map LSP semantic token types to colors
        // See: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_semanticTokens
        match token_type {
            0 => Style::default().fg(Color::Cyan),     // namespace
            1 => Style::default().fg(Color::Yellow),   // type
            2 => Style::default().fg(Color::Yellow),   // class
            3 => Style::default().fg(Color::Yellow),   // enum
            4 => Style::default().fg(Color::Cyan),     // interface
            5 => Style::default().fg(Color::Yellow),   // struct
            6 => Style::default().fg(Color::Magenta),  // typeParameter
            7 => Style::default().fg(Color::White),    // parameter
            8 => Style::default().fg(Color::White),    // variable
            9 => Style::default().fg(Color::Cyan),     // property
            10 => Style::default().fg(Color::Green),   // enumMember
            11 => Style::default().fg(Color::Blue),    // function
            12 => Style::default().fg(Color::Blue),    // method
            13 => Style::default().fg(Color::Magenta), // macro
            14 => Style::default().fg(Color::Magenta), // keyword
            15 => Style::default().fg(Color::Gray),    // comment
            16 => Style::default().fg(Color::Green),   // string
            17 => Style::default().fg(Color::Green),   // number
            18 => Style::default().fg(Color::Magenta), // operator
            _ => Style::default(),
        }
    }

    /// Render line with syntect (fallback)
    fn render_with_syntect(&self, content: &str, language: &str) -> Vec<Span<'_>> {
        if let Some((syntax, syntax_set)) = self.get_syntax(language) {
            let mut highlighter = HighlightLines::new(syntax, &self.theme);

            match highlighter.highlight_line(content, syntax_set) {
                Ok(regions) => regions
                    .iter()
                    .map(|(style, text)| {
                        let fg_color =
                            Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
                        Span::styled(text.to_string(), Style::default().fg(fg_color))
                    })
                    .collect(),
                Err(_) => vec![Span::raw(content.to_string())],
            }
        } else {
            // Unknown language, return plain text
            vec![Span::raw(content.to_string())]
        }
    }
}

impl Default for CodeRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer for CodeRenderer {
    fn render_line(
        &self,
        buffer: &DocumentBuffer,
        line_idx: usize,
        is_current_line: bool,
    ) -> Vec<Span<'_>> {
        let content = buffer.line(line_idx).unwrap_or("");

        if is_current_line {
            return vec![Span::styled(content.to_string(), Style::default())];
        }

        let language = match buffer.document_type() {
            crate::document::DocumentType::Code { language } => language.as_str(),
            _ => return vec![Span::raw(content.to_string())],
        };

        if self.use_semantic_tokens
            && let Some(tokens) = self.semantic_tokens.get(&line_idx)
        {
            return self.render_with_semantic_tokens(content, tokens);
        }

        self.render_with_syntect(content, language)
    }

    fn supports_wysiwyg(&self) -> bool {
        true // Cursor line = source, other lines = highlighted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_renderer_creation() {
        let renderer = CodeRenderer::new();
        assert!(renderer.supports_wysiwyg());
        assert!(renderer.semantic_tokens.is_empty());
    }

    #[test]
    fn test_semantic_tokens_update() {
        let mut renderer = CodeRenderer::new();
        let mut tokens = HashMap::new();
        tokens.insert(
            0,
            vec![SemanticToken {
                start: 0,
                length: 3,
                token_type: 14, // keyword
                modifiers: 0,
            }],
        );

        renderer.set_semantic_tokens(tokens);
        assert_eq!(renderer.semantic_tokens.len(), 1);

        renderer.clear_semantic_tokens();
        assert!(renderer.semantic_tokens.is_empty());
    }
}
