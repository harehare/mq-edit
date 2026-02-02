use std::path::Path;

use miette::Result;

use crate::document::DocumentBuffer;

/// Load a Markdown file into a DocumentBuffer
pub fn load_file(path: impl AsRef<Path>) -> Result<DocumentBuffer> {
    DocumentBuffer::from_file(path)
}

/// Save a DocumentBuffer to its file path
pub fn save_file(buffer: &mut DocumentBuffer) -> Result<()> {
    buffer.save()
}

/// Save a DocumentBuffer to a specific file
pub fn save_file_as(buffer: &mut DocumentBuffer, path: impl AsRef<Path>) -> Result<()> {
    buffer.save_as(path)
}
