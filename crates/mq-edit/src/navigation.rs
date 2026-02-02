use std::path::PathBuf;

/// A location in a file (file path, line, column)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileLocation {
    pub path: PathBuf,
    pub line: usize,
    pub column: usize,
}

impl FileLocation {
    pub fn new(path: PathBuf, line: usize, column: usize) -> Self {
        Self { path, line, column }
    }
}

/// Navigation history manager for tracking jump history (back/forward)
pub struct NavigationHistory {
    /// Stack of previous locations
    history: Vec<FileLocation>,
    /// Current position in history (index)
    current: usize,
}

impl NavigationHistory {
    /// Create a new navigation history
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            current: 0,
        }
    }

    /// Push a new location to history
    /// This clears any forward history if we're not at the end
    pub fn push(&mut self, location: FileLocation) {
        // Don't add duplicate consecutive locations
        if let Some(last) = self.history.last()
            && last == &location
        {
            return;
        }

        // If we're not at the end of history, truncate forward history
        if self.current < self.history.len() {
            self.history.truncate(self.current);
        }

        // Add new location
        self.history.push(location);
        self.current = self.history.len();
    }

    /// Go back in history
    /// Returns the previous location if available
    pub fn back(&mut self) -> Option<&FileLocation> {
        if self.current > 1 {
            self.current -= 1;
            self.history.get(self.current - 1)
        } else {
            None
        }
    }

    /// Go forward in history
    /// Returns the next location if available
    pub fn forward(&mut self) -> Option<&FileLocation> {
        if self.current < self.history.len() {
            self.current += 1;
            self.history.get(self.current - 1)
        } else {
            None
        }
    }

    /// Get the current location
    pub fn current(&self) -> Option<&FileLocation> {
        if self.current > 0 && self.current <= self.history.len() {
            self.history.get(self.current - 1)
        } else {
            None
        }
    }

    /// Check if we can go back
    pub fn can_go_back(&self) -> bool {
        self.current > 1
    }

    /// Check if we can go forward
    pub fn can_go_forward(&self) -> bool {
        self.current < self.history.len()
    }

    /// Get the size of the history
    pub fn len(&self) -> usize {
        self.history.len()
    }

    /// Check if history is empty
    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.history.clear();
        self.current = 0;
    }
}

impl Default for NavigationHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigation_history_creation() {
        let history = NavigationHistory::new();
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
        assert!(!history.can_go_back());
        assert!(!history.can_go_forward());
    }

    #[test]
    fn test_push_location() {
        let mut history = NavigationHistory::new();
        let loc1 = FileLocation::new(PathBuf::from("file1.rs"), 10, 5);

        history.push(loc1.clone());
        assert_eq!(history.len(), 1);
        assert_eq!(history.current(), Some(&loc1));
        assert!(!history.can_go_back());
        assert!(!history.can_go_forward());
    }

    #[test]
    fn test_push_multiple_locations() {
        let mut history = NavigationHistory::new();
        let loc1 = FileLocation::new(PathBuf::from("file1.rs"), 10, 5);
        let loc2 = FileLocation::new(PathBuf::from("file2.rs"), 20, 10);
        let loc3 = FileLocation::new(PathBuf::from("file3.rs"), 30, 15);

        history.push(loc1.clone());
        history.push(loc2.clone());
        history.push(loc3.clone());

        assert_eq!(history.len(), 3);
        assert_eq!(history.current(), Some(&loc3));
        assert!(history.can_go_back());
        assert!(!history.can_go_forward());
    }

    #[test]
    fn test_duplicate_consecutive_locations() {
        let mut history = NavigationHistory::new();
        let loc1 = FileLocation::new(PathBuf::from("file1.rs"), 10, 5);

        history.push(loc1.clone());
        history.push(loc1.clone());
        history.push(loc1.clone());

        assert_eq!(history.len(), 1);
    }

    #[test]
    fn test_back_navigation() {
        let mut history = NavigationHistory::new();
        let loc1 = FileLocation::new(PathBuf::from("file1.rs"), 10, 5);
        let loc2 = FileLocation::new(PathBuf::from("file2.rs"), 20, 10);
        let loc3 = FileLocation::new(PathBuf::from("file3.rs"), 30, 15);

        history.push(loc1.clone());
        history.push(loc2.clone());
        history.push(loc3.clone());

        // Go back to loc2
        assert_eq!(history.back(), Some(&loc2));
        assert_eq!(history.current(), Some(&loc2));
        assert!(history.can_go_back());
        assert!(history.can_go_forward());

        // Go back to loc1
        assert_eq!(history.back(), Some(&loc1));
        assert_eq!(history.current(), Some(&loc1));
        assert!(!history.can_go_back());
        assert!(history.can_go_forward());

        // Try to go back again (should return None)
        assert_eq!(history.back(), None);
    }

    #[test]
    fn test_forward_navigation() {
        let mut history = NavigationHistory::new();
        let loc1 = FileLocation::new(PathBuf::from("file1.rs"), 10, 5);
        let loc2 = FileLocation::new(PathBuf::from("file2.rs"), 20, 10);
        let loc3 = FileLocation::new(PathBuf::from("file3.rs"), 30, 15);

        history.push(loc1.clone());
        history.push(loc2.clone());
        history.push(loc3.clone());

        // Go back twice
        history.back();
        history.back();

        // Go forward to loc2
        assert_eq!(history.forward(), Some(&loc2));
        assert_eq!(history.current(), Some(&loc2));

        // Go forward to loc3
        assert_eq!(history.forward(), Some(&loc3));
        assert_eq!(history.current(), Some(&loc3));

        // Try to go forward again (should return None)
        assert_eq!(history.forward(), None);
    }

    #[test]
    fn test_truncate_forward_history() {
        let mut history = NavigationHistory::new();
        let loc1 = FileLocation::new(PathBuf::from("file1.rs"), 10, 5);
        let loc2 = FileLocation::new(PathBuf::from("file2.rs"), 20, 10);
        let loc3 = FileLocation::new(PathBuf::from("file3.rs"), 30, 15);
        let loc4 = FileLocation::new(PathBuf::from("file4.rs"), 40, 20);

        history.push(loc1.clone());
        history.push(loc2.clone());
        history.push(loc3.clone());

        // Go back twice (from loc3 to loc2 to loc1)
        history.back();
        history.back();

        // Current should be loc1, and we should be able to push loc4
        // This should truncate loc2 and loc3 from history
        history.push(loc4.clone());

        // History should now be: [loc1, loc4]
        assert_eq!(history.len(), 2);
        assert_eq!(history.current(), Some(&loc4));
        assert!(!history.can_go_forward());
    }

    #[test]
    fn test_clear_history() {
        let mut history = NavigationHistory::new();
        let loc1 = FileLocation::new(PathBuf::from("file1.rs"), 10, 5);
        let loc2 = FileLocation::new(PathBuf::from("file2.rs"), 20, 10);

        history.push(loc1);
        history.push(loc2);

        history.clear();

        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
        assert!(!history.can_go_back());
        assert!(!history.can_go_forward());
        assert_eq!(history.current(), None);
    }
}
