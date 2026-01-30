use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// LSP configuration for language servers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspConfig {
    /// Language server configurations by language ID
    #[serde(default)]
    pub servers: HashMap<String, LspServerConfig>,
}

impl Default for LspConfig {
    fn default() -> Self {
        let mut servers = HashMap::new();

        // Add default Rust analyzer configuration
        servers.insert(
            "rust".to_string(),
            LspServerConfig {
                command: "rust-analyzer".to_string(),
                args: vec![],
                enable_completion: true,
                enable_diagnostics: true,
                enable_goto_definition: true,
            },
        );

        // Add default Python language server configuration
        servers.insert(
            "python".to_string(),
            LspServerConfig {
                command: "pyright-langserver".to_string(),
                args: vec!["--stdio".to_string()],
                enable_completion: true,
                enable_diagnostics: true,
                enable_goto_definition: true,
            },
        );

        // Add default MQ language server configuration
        servers.insert(
            "mq".to_string(),
            LspServerConfig {
                command: "mq-lsp".to_string(),
                args: vec![],
                enable_completion: true,
                enable_diagnostics: true,
                enable_goto_definition: true,
            },
        );

        Self { servers }
    }
}

/// Configuration for a specific LSP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServerConfig {
    /// Command to start the LSP server (e.g., "rust-analyzer")
    pub command: String,

    /// Command-line arguments for the server
    #[serde(default)]
    pub args: Vec<String>,

    /// Enable code completion
    #[serde(default = "default_true")]
    pub enable_completion: bool,

    /// Enable diagnostics (errors, warnings)
    #[serde(default = "default_true")]
    pub enable_diagnostics: bool,

    /// Enable go-to-definition
    #[serde(default = "default_true")]
    pub enable_goto_definition: bool,
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

/// Editor display configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorConfig {
    /// Show line numbers in the editor
    #[serde(default = "default_true")]
    pub show_line_numbers: bool,

    /// Highlight the current line
    #[serde(default = "default_true")]
    pub show_current_line_highlight: bool,

    /// Syntax highlighting theme
    #[serde(default = "default_theme")]
    pub theme: String,

    /// Use semantic tokens from LSP for syntax highlighting
    /// When false, falls back to syntect (default: false)
    #[serde(default = "default_false")]
    pub use_semantic_tokens: bool,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            show_line_numbers: true,
            show_current_line_highlight: true,
            theme: default_theme(),
            use_semantic_tokens: false,
        }
    }
}

fn default_theme() -> String {
    "base16-ocean.dark".to_string()
}

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub editor: EditorConfig,
    pub keybindings: Keybindings,
    #[serde(default)]
    pub lsp: LspConfig,
}

impl Config {
    /// Load configuration from file
    pub fn load_from_file(path: impl AsRef<Path>) -> miette::Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| miette::miette!("Failed to read config file: {}", e))?;

        toml::from_str(&content).map_err(|e| miette::miette!("Failed to parse config file: {}", e))
    }

    /// Save configuration to file
    pub fn save_to_file(&self, path: impl AsRef<Path>) -> miette::Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| miette::miette!("Failed to serialize config: {}", e))?;

        std::fs::write(path.as_ref(), content)
            .map_err(|e| miette::miette!("Failed to write config file: {}", e))?;

        Ok(())
    }

    /// Load configuration from default location or use defaults
    pub fn load_or_default() -> Self {
        let config_path = Self::default_config_path();

        if config_path.exists() {
            Self::load_from_file(&config_path).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    /// Get default config file path
    pub fn default_config_path() -> std::path::PathBuf {
        if let Some(config_dir) = dirs::config_dir() {
            config_dir.join("mq").join("edit").join("config.toml")
        } else {
            std::path::PathBuf::from(".mq-edit.toml")
        }
    }
}

/// Keybindings configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keybindings {
    /// Quit application (default: Ctrl+Q)
    pub quit: KeyBinding,

    /// Alternative quit binding (default: Esc)
    pub quit_alt: KeyBinding,

    /// Save file (default: Ctrl+S)
    pub save: KeyBinding,

    /// Toggle file browser (default: Alt+B to avoid VSCode/Zellij conflicts)
    pub toggle_file_browser: KeyBinding,

    /// Alternative toggle file browser (default: F2)
    pub toggle_file_browser_alt: KeyBinding,

    /// Go to definition (default: Ctrl+G)
    pub goto_definition: KeyBinding,

    /// Navigate back in history (default: Ctrl+B)
    pub navigate_back: KeyBinding,

    /// Navigate forward in history (default: Ctrl+Shift+B)
    pub navigate_forward: KeyBinding,

    /// Search (planned for Phase 4)
    pub search: KeyBinding,

    /// Replace (planned for Phase 4)
    pub replace: KeyBinding,

    /// Undo (planned for Phase 4)
    pub undo: KeyBinding,

    /// Redo (planned for Phase 4)
    pub redo: KeyBinding,

    /// Close file browser
    pub close_browser: KeyBinding,

    /// Toggle line numbers display
    pub toggle_line_numbers: KeyBinding,

    /// Toggle current line highlight
    pub toggle_current_line_highlight: KeyBinding,

    /// Go to line (default: Ctrl+G)
    pub goto_line: KeyBinding,
}

impl Default for Keybindings {
    fn default() -> Self {
        Self {
            // Use Ctrl+Q for quit
            quit: KeyBinding {
                code: "q".to_string(),
                modifiers: vec!["ctrl".to_string()],
            },
            // Alternative quit with Esc
            quit_alt: KeyBinding {
                code: "esc".to_string(),
                modifiers: vec![],
            },
            // Ctrl+S is pretty universal for save
            save: KeyBinding {
                code: "s".to_string(),
                modifiers: vec!["ctrl".to_string()],
            },
            // Use Alt+B instead of Ctrl+B to avoid navigation conflicts
            toggle_file_browser: KeyBinding {
                code: "b".to_string(),
                modifiers: vec!["alt".to_string()],
            },
            toggle_file_browser_alt: KeyBinding {
                code: "f2".to_string(),
                modifiers: vec![],
            },
            // Ctrl+D for go to definition
            goto_definition: KeyBinding {
                code: "d".to_string(),
                modifiers: vec!["ctrl".to_string()],
            },
            // Ctrl+B for navigate back (like browser back button)
            navigate_back: KeyBinding {
                code: "b".to_string(),
                modifiers: vec!["ctrl".to_string()],
            },
            // Ctrl+F for navigate forward (like browser forward button)
            navigate_forward: KeyBinding {
                code: "f".to_string(),
                modifiers: vec!["ctrl".to_string()],
            },
            // F3 for search (avoids vim conflicts with Ctrl+F, Ctrl+/, etc.)
            search: KeyBinding {
                code: "f3".to_string(),
                modifiers: vec![],
            },
            // F4 for replace (avoids vim conflicts)
            replace: KeyBinding {
                code: "f4".to_string(),
                modifiers: vec![],
            },
            // Ctrl+Z for undo (standard)
            undo: KeyBinding {
                code: "z".to_string(),
                modifiers: vec!["ctrl".to_string()],
            },
            // Ctrl+Y for redo (standard on Windows/Linux)
            redo: KeyBinding {
                code: "y".to_string(),
                modifiers: vec!["ctrl".to_string()],
            },
            // Esc to close browser (standard)
            close_browser: KeyBinding {
                code: "esc".to_string(),
                modifiers: vec![],
            },
            // Ctrl+L for toggle line numbers
            toggle_line_numbers: KeyBinding {
                code: "l".to_string(),
                modifiers: vec!["ctrl".to_string()],
            },
            // Ctrl+Shift+L for toggle current line highlight
            toggle_current_line_highlight: KeyBinding {
                code: "l".to_string(),
                modifiers: vec!["ctrl".to_string(), "shift".to_string()],
            },
            // Ctrl+G for go to line (like vim)
            goto_line: KeyBinding {
                code: "g".to_string(),
                modifiers: vec!["ctrl".to_string()],
            },
        }
    }
}

/// Represents a key binding with modifiers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBinding {
    pub code: String,
    pub modifiers: Vec<String>,
}

impl KeyBinding {
    /// Check if this keybinding matches a KeyEvent
    pub fn matches(&self, key: &KeyEvent) -> bool {
        // Parse the key code
        let code_matches = match self.code.to_lowercase().as_str() {
            "esc" => matches!(key.code, KeyCode::Esc),
            "enter" => matches!(key.code, KeyCode::Enter),
            "backspace" => matches!(key.code, KeyCode::Backspace),
            "tab" => matches!(key.code, KeyCode::Tab),
            "space" => matches!(key.code, KeyCode::Char(' ')),
            "f1" => matches!(key.code, KeyCode::F(1)),
            "f2" => matches!(key.code, KeyCode::F(2)),
            "f3" => matches!(key.code, KeyCode::F(3)),
            "f4" => matches!(key.code, KeyCode::F(4)),
            "f5" => matches!(key.code, KeyCode::F(5)),
            "f6" => matches!(key.code, KeyCode::F(6)),
            "f7" => matches!(key.code, KeyCode::F(7)),
            "f8" => matches!(key.code, KeyCode::F(8)),
            "f9" => matches!(key.code, KeyCode::F(9)),
            "f10" => matches!(key.code, KeyCode::F(10)),
            "f11" => matches!(key.code, KeyCode::F(11)),
            "f12" => matches!(key.code, KeyCode::F(12)),
            s if s.len() == 1 => {
                if let Some(ch) = s.chars().next() {
                    matches!(key.code, KeyCode::Char(c) if c.to_lowercase().to_string() == ch.to_lowercase().to_string())
                } else {
                    false
                }
            }
            _ => false,
        };

        if !code_matches {
            return false;
        }

        // Check modifiers
        let mut expected_modifiers = KeyModifiers::empty();
        for modifier in &self.modifiers {
            match modifier.to_lowercase().as_str() {
                "ctrl" | "control" => expected_modifiers |= KeyModifiers::CONTROL,
                "shift" => expected_modifiers |= KeyModifiers::SHIFT,
                "alt" => expected_modifiers |= KeyModifiers::ALT,
                _ => {}
            }
        }

        key.modifiers == expected_modifiers
    }

    /// Get a human-readable representation of the keybinding
    pub fn display(&self) -> String {
        let mut parts = Vec::new();

        for modifier in &self.modifiers {
            match modifier.to_lowercase().as_str() {
                "ctrl" | "control" => parts.push("Ctrl".to_string()),
                "shift" => parts.push("Shift".to_string()),
                "alt" => parts.push("Alt".to_string()),
                _ => parts.push(modifier.clone()),
            }
        }

        parts.push(self.code.to_uppercase());
        parts.join("+")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keybinding_matches() {
        let kb = KeyBinding {
            code: "q".to_string(),
            modifiers: vec!["ctrl".to_string()],
        };

        let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
        assert!(kb.matches(&key));

        let key2 = KeyEvent::new(
            KeyCode::Char('q'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert!(!kb.matches(&key2));
    }

    #[test]
    fn test_keybinding_display() {
        let kb = KeyBinding {
            code: "s".to_string(),
            modifiers: vec!["ctrl".to_string()],
        };
        assert_eq!(kb.display(), "Ctrl+S");

        let kb2 = KeyBinding {
            code: "q".to_string(),
            modifiers: vec!["ctrl".to_string()],
        };
        assert_eq!(kb2.display(), "Ctrl+Q");
    }

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.keybindings.save.code, "s");
        assert_eq!(config.keybindings.quit.code, "q");
        assert_eq!(config.keybindings.quit.modifiers.len(), 1);
        assert_eq!(config.keybindings.quit_alt.code, "esc");
        assert_eq!(config.keybindings.quit_alt.modifiers.len(), 0);
    }

    #[test]
    fn test_lsp_config_default() {
        let lsp_config = LspConfig::default();

        // Should have default Rust and Python servers
        assert!(lsp_config.servers.contains_key("rust"));
        assert!(lsp_config.servers.contains_key("python"));

        // Check Rust analyzer config
        let rust_config = &lsp_config.servers["rust"];
        assert_eq!(rust_config.command, "rust-analyzer");
        assert!(rust_config.args.is_empty());
        assert!(rust_config.enable_completion);
        assert!(rust_config.enable_diagnostics);
        assert!(rust_config.enable_goto_definition);

        // Check Python config
        let python_config = &lsp_config.servers["python"];
        assert_eq!(python_config.command, "pyright-langserver");
        assert_eq!(python_config.args, vec!["--stdio"]);
        assert!(python_config.enable_completion);
        assert!(python_config.enable_diagnostics);
        assert!(python_config.enable_goto_definition);
    }

    #[test]
    fn test_lsp_config_serialization() {
        let mut servers = HashMap::new();
        servers.insert(
            "test".to_string(),
            LspServerConfig {
                command: "test-lsp".to_string(),
                args: vec!["--test".to_string()],
                enable_completion: true,
                enable_diagnostics: false,
                enable_goto_definition: true,
            },
        );

        let lsp_config = LspConfig { servers };

        // Serialize to TOML
        let toml_string = toml::to_string(&lsp_config).unwrap();

        // Deserialize back
        let deserialized: LspConfig = toml::from_str(&toml_string).unwrap();

        assert!(deserialized.servers.contains_key("test"));
        let test_config = &deserialized.servers["test"];
        assert_eq!(test_config.command, "test-lsp");
        assert_eq!(test_config.args, vec!["--test"]);
        assert!(test_config.enable_completion);
        assert!(!test_config.enable_diagnostics);
        assert!(test_config.enable_goto_definition);
    }

    #[test]
    fn test_config_with_lsp() {
        let config = Config::default();

        // Should have LSP config
        assert!(!config.lsp.servers.is_empty());
        assert!(config.lsp.servers.contains_key("rust"));
    }

    #[test]
    fn test_lsp_server_config_defaults() {
        // Test that serde defaults work correctly
        let toml = r#"
            command = "my-lsp"
        "#;

        let config: LspServerConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.command, "my-lsp");
        assert!(config.args.is_empty());
        assert!(config.enable_completion);
        assert!(config.enable_diagnostics);
        assert!(config.enable_goto_definition);
    }
}
