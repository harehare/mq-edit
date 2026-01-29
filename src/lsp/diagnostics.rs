use lsp_types::{Diagnostic, DiagnosticSeverity};
use std::collections::HashMap;

/// Manages LSP diagnostics (errors, warnings, hints) for the document
#[derive(Debug, Clone, Default)]
pub struct DiagnosticsManager {
    /// Diagnostics mapped by line number
    diagnostics: HashMap<usize, Vec<Diagnostic>>,
}

impl DiagnosticsManager {
    /// Create a new diagnostics manager
    pub fn new() -> Self {
        Self {
            diagnostics: HashMap::new(),
        }
    }

    /// Update diagnostics from LSP
    pub fn update(&mut self, diagnostics: Vec<Diagnostic>) {
        self.diagnostics.clear();

        for diagnostic in diagnostics {
            let line = diagnostic.range.start.line as usize;
            self.diagnostics.entry(line).or_default().push(diagnostic);
        }
    }

    /// Clear all diagnostics
    pub fn clear(&mut self) {
        self.diagnostics.clear();
    }

    /// Get diagnostics for a specific line
    pub fn get_for_line(&self, line: usize) -> Option<&[Diagnostic]> {
        self.diagnostics.get(&line).map(|v| v.as_slice())
    }

    /// Get all diagnostics
    pub fn all(&self) -> Vec<&Diagnostic> {
        self.diagnostics.values().flatten().collect()
    }

    /// Get error count
    pub fn error_count(&self) -> usize {
        self.all()
            .iter()
            .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
            .count()
    }

    /// Get warning count
    pub fn warning_count(&self) -> usize {
        self.all()
            .iter()
            .filter(|d| d.severity == Some(DiagnosticSeverity::WARNING))
            .count()
    }

    /// Check if there are any diagnostics
    pub fn has_diagnostics(&self) -> bool {
        !self.diagnostics.is_empty()
    }

    /// Get the most severe diagnostic for a line (error > warning > info > hint)
    pub fn most_severe_for_line(&self, line: usize) -> Option<&Diagnostic> {
        let diagnostics = self.get_for_line(line)?;

        // Find error first
        if let Some(error) = diagnostics
            .iter()
            .find(|d| d.severity == Some(DiagnosticSeverity::ERROR))
        {
            return Some(error);
        }

        // Then warning
        if let Some(warning) = diagnostics
            .iter()
            .find(|d| d.severity == Some(DiagnosticSeverity::WARNING))
        {
            return Some(warning);
        }

        // Then info
        if let Some(info) = diagnostics
            .iter()
            .find(|d| d.severity == Some(DiagnosticSeverity::INFORMATION))
        {
            return Some(info);
        }

        // Finally hint
        diagnostics
            .iter()
            .find(|d| d.severity == Some(DiagnosticSeverity::HINT))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{Position, Range};

    fn create_diagnostic(line: u32, severity: DiagnosticSeverity, message: &str) -> Diagnostic {
        Diagnostic {
            range: Range {
                start: Position { line, character: 0 },
                end: Position {
                    line,
                    character: 10,
                },
            },
            severity: Some(severity),
            message: message.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_diagnostics_manager_creation() {
        let manager = DiagnosticsManager::new();
        assert!(!manager.has_diagnostics());
        assert_eq!(manager.error_count(), 0);
        assert_eq!(manager.warning_count(), 0);
    }

    #[test]
    fn test_update_diagnostics() {
        let mut manager = DiagnosticsManager::new();

        let diagnostics = vec![
            create_diagnostic(0, DiagnosticSeverity::ERROR, "Error on line 0"),
            create_diagnostic(0, DiagnosticSeverity::WARNING, "Warning on line 0"),
            create_diagnostic(5, DiagnosticSeverity::WARNING, "Warning on line 5"),
        ];

        manager.update(diagnostics);

        assert!(manager.has_diagnostics());
        assert_eq!(manager.error_count(), 1);
        assert_eq!(manager.warning_count(), 2);
        assert_eq!(manager.get_for_line(0).unwrap().len(), 2);
        assert_eq!(manager.get_for_line(5).unwrap().len(), 1);
        assert!(manager.get_for_line(10).is_none());
    }

    #[test]
    fn test_clear_diagnostics() {
        let mut manager = DiagnosticsManager::new();
        manager.update(vec![create_diagnostic(
            0,
            DiagnosticSeverity::ERROR,
            "Error",
        )]);

        assert!(manager.has_diagnostics());

        manager.clear();

        assert!(!manager.has_diagnostics());
        assert_eq!(manager.error_count(), 0);
    }

    #[test]
    fn test_most_severe_for_line() {
        let mut manager = DiagnosticsManager::new();

        let diagnostics = vec![
            create_diagnostic(0, DiagnosticSeverity::WARNING, "Warning"),
            create_diagnostic(0, DiagnosticSeverity::ERROR, "Error"),
            create_diagnostic(0, DiagnosticSeverity::HINT, "Hint"),
        ];

        manager.update(diagnostics);

        let most_severe = manager.most_severe_for_line(0).unwrap();
        assert_eq!(most_severe.severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(most_severe.message, "Error");
    }
}
