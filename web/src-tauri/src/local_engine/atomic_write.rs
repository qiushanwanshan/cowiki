//! Crash-safe file publish: write a sibling temp file, then rename or link.
//!
//! Adapters and ingest must not truncate an existing destination. A process
//! that dies mid-write only leaves a `.cowiki-*.tmp` sibling, never a
//! half-finished Source or page.

use std::path::{Path, PathBuf};

pub const FILE_ALREADY_EXISTS: &str = "file already exists";

/// Create or replace `path` by renaming a fully written sibling temp file.
pub fn replace_file_atomically(path: &Path, contents: impl AsRef<[u8]>) -> Result<(), String> {
    let temporary = write_sibling_temp(path, contents.as_ref())?;
    if let Err(error) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    Ok(())
}

/// Publish a new file. Fails if `path` already exists, so a concurrent
/// create cannot silently overwrite another writer's completed file.
pub fn create_file_atomically(path: &Path, contents: impl AsRef<[u8]>) -> Result<(), String> {
    let temporary = write_sibling_temp(path, contents.as_ref())?;
    if let Err(error) = std::fs::hard_link(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return if error.kind() == std::io::ErrorKind::AlreadyExists {
            Err(FILE_ALREADY_EXISTS.to_string())
        } else {
            Err(error.to_string())
        };
    }
    std::fs::remove_file(&temporary).map_err(|error| error.to_string())
}

fn write_sibling_temp(path: &Path, contents: &[u8]) -> Result<PathBuf, String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let temporary = sibling_temp_path(path);
    if let Err(error) = std::fs::write(&temporary, contents) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    Ok(temporary)
}

fn sibling_temp_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    path.with_file_name(format!(
        "{file_name}.cowiki-{}.tmp",
        uuid::Uuid::new_v4().simple()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leftover_temps(dir: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains(".cowiki-") && name.ends_with(".tmp"))
            })
            .collect()
    }

    #[test]
    fn replace_writes_the_full_file_and_leaves_no_temp() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("notes.md");
        replace_file_atomically(&path, "complete body\n").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "complete body\n");
        assert!(leftover_temps(temp.path()).is_empty());
    }

    #[test]
    fn replace_does_not_leave_a_truncated_destination() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("source.md");
        std::fs::write(&path, "old source that must not stay truncated").unwrap();
        replace_file_atomically(&path, "replacement that is fully written\n").unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "replacement that is fully written\n"
        );
        assert!(leftover_temps(temp.path()).is_empty());
    }

    #[test]
    fn create_refuses_to_replace_an_existing_file() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("page.md");
        std::fs::write(&path, "original\n").unwrap();
        let error = create_file_atomically(&path, "intruder\n").unwrap_err();
        assert_eq!(error, FILE_ALREADY_EXISTS);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "original\n");
        assert!(leftover_temps(temp.path()).is_empty());
    }

    #[test]
    fn create_makes_missing_parent_directories() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(".cowiki/sources/_encoded/note.md");
        create_file_atomically(&path, "---\ntype: Source\n---\n\nbody\n").unwrap();
        assert!(path.is_file());
        assert!(leftover_temps(path.parent().unwrap()).is_empty());
    }
}
