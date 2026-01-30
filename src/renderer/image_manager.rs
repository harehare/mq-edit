use image::DynamicImage;
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Manages image loading and caching for the editor
pub struct ImageManager {
    /// Cache of loaded images (uses RefCell for interior mutability)
    cache: RefCell<HashMap<PathBuf, DynamicImage>>,
    /// Base path for resolving relative image paths
    base_path: Option<PathBuf>,
    /// Terminal protocol picker (uses RefCell for interior mutability)
    picker: RefCell<Option<Picker>>,
}

impl ImageManager {
    pub fn new() -> Self {
        // Try to initialize picker, but don't fail if it doesn't work
        let picker = Picker::from_termios().ok();

        Self {
            cache: RefCell::new(HashMap::new()),
            base_path: None,
            picker: RefCell::new(picker),
        }
    }

    /// Set the base path for resolving relative image paths
    pub fn set_base_path(&mut self, path: PathBuf) {
        self.base_path = Some(path);
    }

    /// Load an image from the given path (uses interior mutability)
    fn load_image(&self, path: &str) -> Result<DynamicImage, String> {
        let resolved_path = self.resolve_path(path)?;

        // Check if image is already cached
        let mut cache = self.cache.borrow_mut();
        if !cache.contains_key(&resolved_path) {
            // Load the image
            let img =
                image::open(&resolved_path).map_err(|e| format!("Failed to load image: {}", e))?;
            cache.insert(resolved_path.clone(), img);
        }

        cache
            .get(&resolved_path)
            .cloned()
            .ok_or_else(|| "Image not found in cache".to_string())
    }

    /// Resolve a path (absolute or relative to base path)
    fn resolve_path(&self, path: &str) -> Result<PathBuf, String> {
        let path = Path::new(path);

        // If absolute path, use as-is
        if path.is_absolute() {
            return Ok(path.to_path_buf());
        }

        // If relative, resolve against base path
        if let Some(base) = &self.base_path {
            let mut resolved = base.clone();
            resolved.pop(); // Remove filename to get directory
            resolved.push(path);
            Ok(resolved)
        } else {
            // No base path, use current directory
            std::env::current_dir()
                .map(|cwd| cwd.join(path))
                .map_err(|e| format!("Failed to get current directory: {}", e))
        }
    }

    /// Check if an image exists and can be loaded
    pub fn can_load_image(&self, path: &str) -> bool {
        self.load_image(path).is_ok()
    }

    /// Create a resized protocol for rendering
    pub fn create_protocol(&self, path: &str) -> Result<Box<dyn StatefulProtocol>, String> {
        // First, load the image
        let img = self.load_image(path)?;

        // Then get the picker
        let mut picker = self.picker.borrow_mut();
        let picker_ref = picker
            .as_mut()
            .ok_or_else(|| "Terminal protocol not supported".to_string())?;

        Ok(picker_ref.new_resize_protocol(img))
    }

    /// Check if terminal supports image rendering
    pub fn supports_images(&self) -> bool {
        self.picker.borrow().is_some()
    }

    /// Clear the image cache
    pub fn clear_cache(&self) {
        self.cache.borrow_mut().clear();
    }

    /// Get image dimensions
    pub fn get_dimensions(&self, path: &str) -> Result<(u32, u32), String> {
        let img = self.load_image(path)?;
        Ok((img.width(), img.height()))
    }
}

impl Default for ImageManager {
    fn default() -> Self {
        Self::new()
    }
}
