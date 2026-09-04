use std::path::Path;

pub const MAX_SOURCE_FILE_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_ARCHIVE_ENTRY_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_ARCHIVE_TOTAL_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_ARCHIVE_ENTRIES: usize = 20_000;
pub const MAX_EXTRACTED_TEXT_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_HTML_RESPONSE_BYTES: u64 = 5 * 1024 * 1024;
pub const MAX_HTML_REDIRECTS: usize = 5;
pub const HTML_FETCH_TIMEOUT_SECS: u64 = 20;
pub const HTML_CONNECT_TIMEOUT_SECS: u64 = 10;

pub fn validate_file_size(path: &Path, max_bytes: u64) -> Result<(), String> {
    let bytes = std::fs::metadata(path)
        .map_err(|error| format!("cannot inspect source file: {error}"))?
        .len();
    if bytes > max_bytes {
        return Err(format!(
            "source file is too large ({bytes} bytes; maximum is {max_bytes})"
        ));
    }
    Ok(())
}

pub fn validate_archive_limits(
    path: &Path,
    max_entry_bytes: u64,
    max_total_bytes: u64,
    max_entries: usize,
) -> Result<(), String> {
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|error| format!("cannot open archive: {error}"))?;
    if archive.len() > max_entries {
        return Err(format!(
            "archive has too many entries ({}; maximum is {max_entries})",
            archive.len()
        ));
    }

    let mut total_bytes = 0_u64;
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|error| format!("cannot inspect archive entry: {error}"))?;
        let entry_bytes = entry.size();
        if entry_bytes > max_entry_bytes {
            return Err(format!(
                "archive entry '{}' is too large ({entry_bytes} bytes; maximum is {max_entry_bytes})",
                entry.name()
            ));
        }
        total_bytes = total_bytes
            .checked_add(entry_bytes)
            .ok_or_else(|| "archive expanded size overflowed".to_string())?;
        if total_bytes > max_total_bytes {
            return Err(format!(
                "archive expands to too much data ({total_bytes} bytes; maximum is {max_total_bytes})"
            ));
        }
    }
    Ok(())
}

pub fn validate_extracted_text(text: &str, max_bytes: usize) -> Result<(), String> {
    if text.len() > max_bytes {
        return Err(format!(
            "extracted text is too large ({} bytes; maximum is {max_bytes})",
            text.len()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::test_support::write_minimal_pptx;

    #[test]
    fn rejects_input_larger_than_the_configured_limit() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("large.txt");
        std::fs::write(&path, b"123456789").unwrap();

        let error = validate_file_size(&path, 8).unwrap_err();
        assert!(error.contains("too large"));
    }

    #[test]
    fn rejects_archive_entries_that_expand_past_the_configured_limit() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("large.pptx");
        write_minimal_pptx(&path, &["This slide expands beyond a tiny test limit"]);

        let error = validate_archive_limits(&path, 8, 1_024, 10).unwrap_err();
        assert!(error.contains("archive entry"));
    }

    #[test]
    fn rejects_extracted_text_larger_than_the_configured_limit() {
        let error = validate_extracted_text("123456789", 8).unwrap_err();
        assert!(error.contains("extracted text"));
    }
}
