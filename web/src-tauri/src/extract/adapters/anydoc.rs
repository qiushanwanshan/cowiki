//! Firecrawl AnyDoc adapter — broad office/PDF → GFM Markdown in-process.

use std::path::Path;
use std::time::Instant;

use super::output::AdapterOutput;
use crate::extract::contract::{DocumentMeta, ExtractResult, Provenance, SourceFormat};

pub const ANYDOC_EXTRACTOR: &str = "anydoc";
pub const ANYDOC_EXTRACTOR_VERSION: &str = "0.2.4";
pub const ANYDOC_SETTINGS_HASH: &str = "gfm-markdown";

/// Formats with no builtin adapter; AnyDoc is the first attempt.
pub const ANYDOC_PRIMARY_EXTENSIONS: &[&str] = &[
    "doc", "rtf", "epub", "ppt", "pps", "pot", "docm", "pptm", "ppsx", "ppsm", "xlsm", "xlsb",
];

pub fn is_anydoc_primary(extension: &str) -> bool {
    ANYDOC_PRIMARY_EXTENSIONS.contains(&extension)
}

pub fn extract_anydoc(path: &Path, format: SourceFormat) -> Result<ExtractResult, String> {
    let started = Instant::now();
    let markdown = anydoc::to_markdown(path).map_err(map_anydoc_error)?;
    let output = AdapterOutput::from_text(markdown);
    let mut result =
        ExtractResult::from_adapter(output.text, format, output.stats, provenance(started));
    result.document = DocumentMeta {
        format,
        ..DocumentMeta::default()
    };
    Ok(result)
}

pub fn anydoc_needs_ocr(error: &anydoc::ConvertError) -> Option<(Vec<u32>, u32)> {
    match error {
        anydoc::ConvertError::NeedsOcr { pages, page_count } => Some((pages.clone(), *page_count)),
        _ => None,
    }
}

fn provenance(started: Instant) -> Provenance {
    let mut provenance = Provenance::local_fast(
        ANYDOC_EXTRACTOR,
        ANYDOC_EXTRACTOR_VERSION,
        started.elapsed().as_millis() as u64,
    );
    provenance.settings_hash = ANYDOC_SETTINGS_HASH.to_string();
    provenance
}

fn map_anydoc_error(error: anydoc::ConvertError) -> String {
    if let Some((pages, page_count)) = anydoc_needs_ocr(&error) {
        return format!(
            "document needs OCR for {}/{} page(s): {:?}",
            pages.len(),
            page_count,
            pages
        );
    }
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_rtf_via_anydoc() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("note.rtf");
        std::fs::write(
            &path,
            r"{\rtf1\ansi\deff0{\fonttbl{\f0 Helvetica;}}\f0\fs24 AnyDoc RTF text.}",
        )
        .unwrap();
        let result = extract_anydoc(&path, SourceFormat::Text).unwrap();
        assert!(result.text.contains("AnyDoc RTF text"));
        assert_eq!(result.provenance.extractor, ANYDOC_EXTRACTOR);
    }
}
