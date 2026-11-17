//! Confined markdown file storage for memories and documents (ADR-0059).
//!
//! Every path handed to this adapter is relative to the data directory. The
//! adapter rejects absolute paths, `..`, symlinks, and writes outside the base
//! directory.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

use crate::app::AppError;
use crate::app::storage::StorageError;
use crate::infra::paths::{PathError, ensure_private_dir};

/// Largest accepted memory file. 2,000 characters can use up to 8 KiB in UTF-8.
pub const MAX_MEMORY_BYTES: usize = 8 * 1024;

/// Largest accepted document.
pub const MAX_DOCUMENT_BYTES: usize = 1024 * 1024;

/// The two root folders under the data directory.
pub const MEMORIES: &str = "memories";
pub const DOCUMENTS: &str = "documents";

/// The three entity folder kinds.
pub const AGENTS: &str = "agents";
pub const PROJECTS: &str = "projects";
pub const LAB: &str = "lab";

#[derive(Debug, Clone)]
pub struct FileStore {
    base: PathBuf,
}

impl FileStore {
    pub fn new(base: PathBuf) -> Self {
        Self { base }
    }

    pub fn base(&self) -> &Path {
        &self.base
    }

    /// Creates the two roots and their entity subfolders.
    pub fn ensure_layout(&self) -> Result<(), FileStoreError> {
        // The data root is private and protects every nested entity folder.
        ensure_private_dir(&self.base).map_err(map_path_error)?;
        for root in [MEMORIES, DOCUMENTS] {
            fs::create_dir_all(self.base.join(root).join(AGENTS))?;
            fs::create_dir_all(self.base.join(root).join(PROJECTS))?;
            fs::create_dir_all(self.base.join(root).join(LAB))?;
        }
        Ok(())
    }

    /// Writes `content` atomically to a confined relative path.
    pub fn write(
        &self,
        relative: &str,
        content: &str,
        max_bytes: usize,
    ) -> Result<(), FileStoreError> {
        if content.len() > max_bytes {
            return Err(FileStoreError::TooLarge);
        }
        let destination = self.resolve_for_write(relative)?;
        let parent = destination.parent().ok_or(FileStoreError::InvalidPath)?;
        let temporary = parent.join(format!(".tmp-{}", uuid::Uuid::new_v4().simple()));
        let result = (|| -> Result<(), FileStoreError> {
            let mut file = private_file(&temporary)?;
            file.write_all(content.as_bytes())?;
            file.sync_all()?;
            fs::rename(&temporary, &destination)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    /// Reads a confined relative path, bounded by `max_bytes`.
    pub fn read(&self, relative: &str, max_bytes: usize) -> Result<String, FileStoreError> {
        let path = self.resolve_existing(relative)?;
        let metadata = fs::metadata(&path)?;
        if !metadata.is_file() {
            return Err(FileStoreError::NotFound);
        }
        if metadata.len() > max_bytes as u64 {
            return Err(FileStoreError::TooLarge);
        }
        let file = File::open(&path)?;
        let mut bytes = Vec::new();
        file.take(max_bytes as u64 + 1).read_to_end(&mut bytes)?;
        String::from_utf8(bytes).map_err(|_| FileStoreError::NotUtf8)
    }

    /// Deletes a confined relative file. A missing file is not an error.
    pub fn delete(&self, relative: &str) -> Result<(), FileStoreError> {
        match self.resolve_existing(relative) {
            Ok(path) => match fs::remove_file(path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(FileStoreError::Io(error.kind())),
            },
            Err(FileStoreError::NotFound) => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// Imports a markdown source into a confined relative destination.
    pub fn import(
        &self,
        source: &Path,
        relative: &str,
        max_bytes: usize,
    ) -> Result<(), FileStoreError> {
        let metadata = fs::symlink_metadata(source).map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => FileStoreError::NotFound,
            kind => FileStoreError::Io(kind),
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(FileStoreError::NotMarkdown);
        }
        if !source
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
        {
            return Err(FileStoreError::NotMarkdown);
        }
        if metadata.len() > max_bytes as u64 {
            return Err(FileStoreError::TooLarge);
        }
        let file = File::open(source)?;
        let mut bytes = Vec::new();
        file.take(max_bytes as u64 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > max_bytes {
            return Err(FileStoreError::TooLarge);
        }
        let content = String::from_utf8(bytes).map_err(|_| FileStoreError::NotUtf8)?;
        self.write(relative, &content, max_bytes)
    }

    /// Removes both memory and document folders for one entity. Best effort.
    pub fn remove_entity(&self, kind: &str, folder: &str) {
        if !matches!(kind, AGENTS | PROJECTS | LAB) || !safe_segment(folder) {
            return;
        }
        for root in [MEMORIES, DOCUMENTS] {
            let path = self.base.join(root).join(kind).join(folder);
            let _ = fs::remove_dir_all(path);
        }
    }

    fn resolve_existing(&self, relative: &str) -> Result<PathBuf, FileStoreError> {
        let relative = validate_relative(relative)?;
        let candidate = self.base.join(relative);
        let canonical = match fs::canonicalize(&candidate) {
            Ok(path) => path,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(FileStoreError::NotFound);
            }
            Err(error) => return Err(FileStoreError::Io(error.kind())),
        };
        self.ensure_inside(&canonical)?;
        Ok(canonical)
    }

    fn resolve_for_write(&self, relative: &str) -> Result<PathBuf, FileStoreError> {
        let relative = validate_relative(relative)?;
        let candidate = self.base.join(relative);
        let parent = candidate.parent().ok_or(FileStoreError::InvalidPath)?;
        // The data root is private; entity subfolders are protected by it and
        // may be created with the process umask.
        fs::create_dir_all(parent)?;
        let parent = fs::canonicalize(parent)?;
        self.ensure_inside(&parent)?;
        let name = candidate.file_name().ok_or(FileStoreError::InvalidPath)?;
        let destination = parent.join(name);
        if fs::symlink_metadata(&destination)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return Err(FileStoreError::Symlink);
        }
        Ok(destination)
    }

    fn ensure_inside(&self, path: &Path) -> Result<(), FileStoreError> {
        let base = fs::canonicalize(&self.base)?;
        if path.starts_with(base) {
            Ok(())
        } else {
            Err(FileStoreError::OutsideBase)
        }
    }
}

fn validate_relative(relative: &str) -> Result<&Path, FileStoreError> {
    let path = Path::new(relative);
    if relative.trim().is_empty() || path.is_absolute() {
        return Err(FileStoreError::InvalidPath);
    }
    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(FileStoreError::InvalidPath);
        }
    }
    Ok(path)
}

fn safe_segment(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && !value.contains('/')
        && !value.contains('\\')
        && !value.chars().any(char::is_control)
}

fn private_file(path: &Path) -> Result<File, FileStoreError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .map_err(|error| FileStoreError::Io(error.kind()))
}

fn map_path_error(error: PathError) -> FileStoreError {
    match error {
        PathError::Io { kind, .. } => FileStoreError::Io(kind),
        PathError::NotPrivate { .. } | PathError::NotADirectory { .. } => {
            FileStoreError::Io(io::ErrorKind::PermissionDenied)
        }
        _ => FileStoreError::Io(io::ErrorKind::Other),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FileStoreError {
    #[error("the file path is invalid.")]
    InvalidPath,
    #[error("the file path escapes the data directory.")]
    OutsideBase,
    #[error("symbolic links are not accepted.")]
    Symlink,
    #[error("the file does not exist.")]
    NotFound,
    #[error("the file is too large.")]
    TooLarge,
    #[error("only UTF-8 markdown files are accepted.")]
    NotMarkdown,
    #[error("the file is not valid UTF-8.")]
    NotUtf8,
    #[error("the file operation failed: {0:?}.")]
    Io(io::ErrorKind),
}

impl From<io::Error> for FileStoreError {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::NotFound => Self::NotFound,
            kind => Self::Io(kind),
        }
    }
}

impl From<FileStoreError> for StorageError {
    fn from(error: FileStoreError) -> Self {
        match error {
            FileStoreError::Io(kind) => StorageError::File { kind },
            _ => StorageError::Operation,
        }
    }
}

impl From<FileStoreError> for AppError {
    fn from(error: FileStoreError) -> Self {
        match error {
            FileStoreError::InvalidPath | FileStoreError::OutsideBase => AppError::Precondition {
                problem: "the document path is outside the data file store.".to_owned(),
                hint: "Use a relative path inside the data directory.".to_owned(),
            },
            FileStoreError::Symlink => AppError::Precondition {
                problem: "symbolic links are not accepted in the file store.".to_owned(),
                hint: "Replace the link with a regular markdown file.".to_owned(),
            },
            FileStoreError::NotFound => AppError::Precondition {
                problem: "the source file does not exist.".to_owned(),
                hint: "Check the source path and retry.".to_owned(),
            },
            FileStoreError::TooLarge => AppError::Validation {
                problem: "the file is larger than the accepted limit.".to_owned(),
                hint: "Use a smaller markdown file.".to_owned(),
            },
            FileStoreError::NotMarkdown => AppError::Usage {
                message: "the source must be a UTF-8 `.md` file.".to_owned(),
            },
            FileStoreError::NotUtf8 => AppError::Validation {
                problem: "the markdown file is not valid UTF-8.".to_owned(),
                hint: "Save the file as UTF-8 and retry.".to_owned(),
            },
            FileStoreError::Io(kind) => AppError::Io {
                context: "accessing the data file store",
                kind,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, FileStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = FileStore::new(dir.path().join("data"));
        store.ensure_layout().unwrap();
        (dir, store)
    }

    #[test]
    fn memory_and_document_roots_have_entity_folders() {
        let (_dir, store) = store();
        for root in [MEMORIES, DOCUMENTS] {
            for kind in [AGENTS, PROJECTS, LAB] {
                assert!(store.base().join(root).join(kind).is_dir());
            }
        }
    }

    #[test]
    fn writes_and_reads_confined_markdown() {
        let (_dir, store) = store();
        let path = "memories/lab/payments/1c2d3e4f.md";
        store
            .write(path, "# Keep tests green", MAX_MEMORY_BYTES)
            .unwrap();
        assert_eq!(
            store.read(path, MAX_MEMORY_BYTES).unwrap(),
            "# Keep tests green"
        );
        store.delete(path).unwrap();
        assert_eq!(
            store.read(path, MAX_MEMORY_BYTES),
            Err(FileStoreError::NotFound)
        );
    }

    #[test]
    fn traversal_and_absolute_paths_are_rejected() {
        let (_dir, store) = store();
        for path in ["../escape.md", "/tmp/escape.md", "memories/../escape.md"] {
            assert_eq!(
                store.write(path, "x", MAX_MEMORY_BYTES),
                Err(FileStoreError::InvalidPath)
            );
        }
    }

    #[test]
    fn imports_only_markdown_regular_files() {
        let (dir, store) = store();
        let source = dir.path().join("source.md");
        fs::write(&source, "# Imported").unwrap();
        store
            .import(
                &source,
                "documents/lab/payments/imported.md",
                MAX_DOCUMENT_BYTES,
            )
            .unwrap();
        assert_eq!(
            store
                .read("documents/lab/payments/imported.md", MAX_DOCUMENT_BYTES)
                .unwrap(),
            "# Imported"
        );
        let text = dir.path().join("source.txt");
        fs::write(&text, "x").unwrap();
        assert_eq!(
            store.import(&text, "documents/lab/payments/x.md", MAX_DOCUMENT_BYTES),
            Err(FileStoreError::NotMarkdown)
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_destinations_are_rejected() {
        use std::os::unix::fs::symlink;
        let (dir, store) = store();
        let outside = dir.path().join("outside.md");
        fs::write(&outside, "x").unwrap();
        let destination = store.base().join("documents/lab/payments/link.md");
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        symlink(&outside, &destination).unwrap();
        assert_eq!(
            store.write("documents/lab/payments/link.md", "new", MAX_DOCUMENT_BYTES),
            Err(FileStoreError::Symlink)
        );
        assert_eq!(fs::read_to_string(outside).unwrap(), "x");
    }
}
