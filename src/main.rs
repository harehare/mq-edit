use std::io;
use std::time::Duration;

use clap::{Parser, Subcommand};
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use miette::Result;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
};

use mq_edit::{
    App, Config,
    renderer::CodeRenderer,
    ui::{
        CompletionPopup, EditorWidget, FileBrowserWidget, GotoLineDialog, QuitDialog, SaveAsDialog,
        SearchDialog, SearchMode, StatusBar,
    },
};

/// A terminal-based Markdown editor with WYSIWYG rendering
#[derive(Parser)]
#[command(name = "mq-edit")]
#[command(version, about, long_about = None)]
struct Cli {
    /// File to open
    file: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize default configuration file
    InitConfig,
    /// List available syntax highlighting themes
    ListThemes,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle subcommands
    if let Some(command) = cli.command {
        match command {
            Commands::InitConfig => {
                return init_config();
            }
            Commands::ListThemes => {
                return list_themes();
            }
        }
    }

    let mut app = if let Some(file_path) = cli.file {
        App::from_file(&file_path)?
    } else {
        App::new()
    };

    // Setup terminal
    enable_raw_mode().map_err(|e| miette::miette!("Failed to enable raw mode: {}", e))?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)
        .map_err(|e| miette::miette!("Failed to enter alternate screen: {}", e))?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal =
        Terminal::new(backend).map_err(|e| miette::miette!("Failed to create terminal: {}", e))?;

    // Run app
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode().map_err(|e| miette::miette!("Failed to disable raw mode: {}", e))?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .map_err(|e| miette::miette!("Failed to leave alternate screen: {}", e))?;
    terminal
        .show_cursor()
        .map_err(|e| miette::miette!("Failed to show cursor: {}", e))?;

    if let Err(e) = res {
        eprintln!("Error: {}", e);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    loop {
        // Draw UI
        terminal
            .draw(|f| {
                let main_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(1),    // Main area (editor + optional file browser)
                        Constraint::Length(1), // Status bar
                    ])
                    .split(f.area());

                // Split main area horizontally if file browser is visible
                let (editor_area, file_browser_area) = if app.is_file_browser_visible() {
                    let chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([
                            Constraint::Percentage(15), // File browser
                            Constraint::Percentage(85), // Editor
                        ])
                        .split(main_chunks[0]);
                    (chunks[1], Some(chunks[0]))
                } else {
                    (main_chunks[0], None)
                };

                // Render file browser if visible
                if let Some(area) = file_browser_area
                    && let Some(tree) = app.file_tree()
                {
                    let file_browser = FileBrowserWidget::new(tree);
                    f.render_widget(file_browser, area);
                }

                // Render editor
                let editor = EditorWidget::new(app.buffer())
                    .with_scroll(app.scroll_offset())
                    .with_code_renderer(app.code_renderer())
                    .with_image_manager(app.image_manager())
                    .with_diagnostics(app.diagnostics_manager())
                    .with_line_numbers(app.show_line_numbers())
                    .with_current_line_highlight(app.show_current_line_highlight());
                f.render_widget(editor, editor_area);

                // Render status bar
                let mut status_bar =
                    StatusBar::new(app.buffer()).with_diagnostics(app.diagnostics_manager());
                if app.show_quit_dialog() {
                    status_bar =
                        status_bar.with_warning("Unsaved changes! Press Y to quit, N to cancel");
                }
                f.render_widget(status_bar, main_chunks[1]);

                // Render completion popup if visible
                if app.show_completion() {
                    let items = app.filtered_completion_items();
                    if !items.is_empty() {
                        let cursor = app.buffer().cursor();
                        let gutter_width = app.line_number_gutter_width();
                        // Use display width instead of column count for correct positioning
                        let display_width = app
                            .buffer()
                            .display_width_to_column(cursor.line, cursor.column);
                        let cursor_x = display_width as u16 + gutter_width;
                        let cursor_y = (cursor.line - app.scroll_offset()) as u16;

                        let popup_rect = CompletionPopup::calculate_rect(
                            editor_area.x + cursor_x,
                            editor_area.y + cursor_y,
                            editor_area,
                        );

                        let popup = CompletionPopup::new(items, app.completion_selected());
                        f.render_widget(popup, popup_rect);
                    }
                }

                // Render quit confirmation dialog if visible
                if app.show_quit_dialog() {
                    let dialog = QuitDialog::new();
                    f.render_widget(dialog, f.area());
                }

                // Render search dialog if visible
                if app.show_search_dialog() {
                    let mut search_dialog = SearchDialog::new(
                        app.search_query(),
                        app.search_match_count(),
                        app.search_current_index(),
                    )
                    .with_active_field(app.search_active_field());

                    if app.search_mode() == SearchMode::Replace {
                        search_dialog = search_dialog.with_replace(app.replace_query());
                    }

                    f.render_widget(search_dialog, f.area());
                }

                // Render save-as dialog if visible
                if app.show_save_as_dialog() {
                    let save_as_dialog = SaveAsDialog::new(app.save_as_filename());
                    f.render_widget(save_as_dialog, f.area());
                }

                // Render goto line dialog if visible
                if app.show_goto_line_dialog() {
                    let goto_line_dialog = GotoLineDialog::new(
                        app.goto_line_input(),
                        app.buffer().cursor().line,
                        app.buffer().line_count(),
                    );
                    f.render_widget(goto_line_dialog, f.area());
                }

                // Set cursor position (only when file browser is not visible and no dialog)
                if !app.is_file_browser_visible() {
                    let cursor = app.buffer().cursor();
                    let gutter_width = app.line_number_gutter_width();
                    // Use display width instead of column count for correct positioning
                    let display_width = app
                        .buffer()
                        .display_width_to_column(cursor.line, cursor.column);
                    let cursor_x = display_width as u16 + gutter_width;
                    let cursor_y = (cursor.line - app.scroll_offset()) as u16;
                    if cursor_y < editor_area.height {
                        f.set_cursor_position((editor_area.x + cursor_x, editor_area.y + cursor_y));
                    }
                }
            })
            .map_err(|e| miette::miette!("Failed to draw terminal: {}", e))?;

        // Handle input
        if event::poll(Duration::from_millis(100))
            .map_err(|e| miette::miette!("Failed to poll events: {}", e))?
        {
            match event::read().map_err(|e| miette::miette!("Failed to read event: {}", e))? {
                Event::Key(key) => {
                    app.handle_key(key)?;
                }
                Event::Paste(text) => {
                    app.handle_paste(text)?;
                }
                _ => {}
            }
        }

        // Poll LSP events
        app.poll_lsp_events();

        // Check if should quit
        if app.should_quit() {
            break;
        }
    }

    Ok(())
}
/// Initialize default configuration file
fn init_config() -> Result<()> {
    let config = Config::default();
    let config_path = Config::default_config_path();

    // Create config directory if it doesn't exist
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| miette::miette!("Failed to create config directory: {}", e))?;
    }

    if config_path.exists() {
        eprintln!("Config file already exists at: {}", config_path.display());
        eprintln!("Remove it first or edit it manually.");
        return Ok(());
    }

    config.save_to_file(&config_path)?;
    println!(
        "Created default config file at: {}\n\n\
         Default keybindings:\n\
         Quit:                {}\n\
         Save:                {}\n\
         Toggle File Browser: {}\n\
         Close Browser:       {}\n\n\
         You can edit this file to customize your keybindings.",
        config_path.display(),
        config.keybindings.quit.display(),
        config.keybindings.save.display(),
        config.keybindings.toggle_file_browser.display(),
        config.keybindings.close_browser.display()
    );

    Ok(())
}

/// List available syntax highlighting themes
fn list_themes() -> Result<()> {
    let themes = CodeRenderer::available_themes();
    let theme_list = themes
        .iter()
        .enumerate()
        .map(|(i, theme)| format!("  {}. {}", i + 1, theme))
        .collect::<Vec<_>>()
        .join("\n");

    println!(
        "Available syntax highlighting themes:\n\n\
         {}\n\n\
         To use a theme, add this to your config file:\n\
         [editor]\n\
         theme = \"base16-ocean.dark\"\n\n\
         Config file location: {}\n\
         Run 'mq-edit --init-config' to create a default config file.",
        theme_list,
        Config::default_config_path().display()
    );

    Ok(())
}
