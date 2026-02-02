use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Widget},
};
use std::path::{Path, PathBuf};

/// Represents an item in the file tree
#[derive(Debug, Clone)]
pub struct FileTreeItem {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub depth: usize,
    pub expanded: bool,
}

impl FileTreeItem {
    pub fn new(path: PathBuf, depth: usize) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        let is_dir = path.is_dir();

        Self {
            path,
            name,
            is_dir,
            depth,
            expanded: false,
        }
    }
}

/// File tree structure for navigating directories
pub struct FileTree {
    items: Vec<FileTreeItem>,
    selected_index: usize,
    root_path: PathBuf,
    gitignore: Option<Gitignore>,
}

impl FileTree {
    /// Create a new file tree from a root directory
    pub fn new(root_path: impl AsRef<Path>) -> Self {
        let root_path = root_path.as_ref().to_path_buf();
        let gitignore = Self::load_gitignore(&root_path);
        let mut tree = Self {
            items: Vec::new(),
            selected_index: 0,
            root_path: root_path.clone(),
            gitignore,
        };

        tree.load_directory(&root_path, 0);
        tree
    }

    /// Load .gitignore from root directory
    fn load_gitignore(root_path: &Path) -> Option<Gitignore> {
        let gitignore_path = root_path.join(".gitignore");
        if gitignore_path.exists() {
            let mut builder = GitignoreBuilder::new(root_path);
            builder.add(&gitignore_path);
            builder.build().ok()
        } else {
            None
        }
    }

    /// Check if a path should be ignored
    fn is_ignored(&self, path: &Path) -> bool {
        if let Some(ref gitignore) = self.gitignore {
            gitignore.matched(path, path.is_dir()).is_ignore()
        } else {
            false
        }
    }

    /// Load directory contents at a specific depth
    fn load_directory(&mut self, path: &Path, depth: usize) {
        if !path.is_dir() {
            return;
        }

        let mut entries: Vec<_> = std::fs::read_dir(path)
            .ok()
            .map(|entries| entries.filter_map(|e| e.ok()).map(|e| e.path()).collect())
            .unwrap_or_default();

        // Sort: directories first, then files, alphabetically
        entries.sort_by(|a, b| match (a.is_dir(), b.is_dir()) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.file_name().cmp(&b.file_name()),
        });

        for entry in entries {
            // Skip files/directories matching .gitignore
            if self.is_ignored(&entry) {
                continue;
            }

            self.items.push(FileTreeItem::new(entry, depth));
        }
    }

    /// Get all items
    pub fn items(&self) -> &[FileTreeItem] {
        &self.items
    }

    /// Get selected index
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Get selected item
    pub fn selected_item(&self) -> Option<&FileTreeItem> {
        self.items.get(self.selected_index)
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        if self.selected_index + 1 < self.items.len() {
            self.selected_index += 1;
        }
    }

    /// Toggle directory expansion
    pub fn toggle_expand(&mut self) {
        if let Some(item) = self.items.get_mut(self.selected_index)
            && item.is_dir
        {
            item.expanded = !item.expanded;

            if item.expanded {
                // Load subdirectory contents
                let path = item.path.clone();
                let depth = item.depth + 1;
                let insert_pos = self.selected_index + 1;

                let mut new_items = Vec::new();
                let mut entries: Vec<_> = std::fs::read_dir(&path)
                    .ok()
                    .map(|entries| entries.filter_map(|e| e.ok()).map(|e| e.path()).collect())
                    .unwrap_or_default();

                entries.sort_by(|a, b| match (a.is_dir(), b.is_dir()) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.file_name().cmp(&b.file_name()),
                });

                for entry in entries {
                    if let Some(name) = entry.file_name()
                        && name.to_string_lossy().starts_with('.')
                    {
                        continue;
                    }

                    // Skip files/directories matching .gitignore
                    if self.is_ignored(&entry) {
                        continue;
                    }

                    new_items.push(FileTreeItem::new(entry, depth));
                }

                // Insert new items after the current directory
                for (i, item) in new_items.into_iter().enumerate() {
                    self.items.insert(insert_pos + i, item);
                }
            } else {
                // Collapse: remove all child items
                let item_depth = item.depth;
                let mut remove_count = 0;

                for i in (self.selected_index + 1)..self.items.len() {
                    if self.items[i].depth <= item_depth {
                        break;
                    }
                    remove_count += 1;
                }

                for _ in 0..remove_count {
                    self.items.remove(self.selected_index + 1);
                }
            }
        }
    }

    /// Refresh the tree
    pub fn refresh(&mut self) {
        self.items.clear();
        self.selected_index = 0;
        self.load_directory(&self.root_path.clone(), 0);
    }
}

/// File browser widget
pub struct FileBrowserWidget<'a> {
    tree: &'a FileTree,
    title: &'a str,
}

impl<'a> FileBrowserWidget<'a> {
    pub fn new(tree: &'a FileTree) -> Self {
        Self {
            tree,
            title: "Files",
        }
    }

    pub fn with_title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }
}

impl Widget for FileBrowserWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let items: Vec<ListItem> = self
            .tree
            .items()
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                let indent = "  ".repeat(item.depth);
                let icon = if item.is_dir {
                    if item.expanded { "▾ " } else { "▸ " }
                } else {
                    "· "
                };

                let style = if idx == self.tree.selected_index() {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else if item.is_dir {
                    Style::default().fg(Color::Blue)
                } else {
                    Style::default().fg(Color::White)
                };

                let content = format!("{}{}{}", indent, icon, item.name);
                ListItem::new(Line::from(vec![Span::styled(content, style)]))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(self.title)
                    .border_style(Style::default().fg(Color::Gray)),
            )
            .style(Style::default().fg(Color::White).bg(Color::Black));

        list.render(area, buf);
    }
}
