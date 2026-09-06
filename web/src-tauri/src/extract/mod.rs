//! Plain-text extraction for non-Markdown source files ingested into a Space.
//!
//! Each supported format gets a best-effort conversion to readable text so
//! it can be stored as a normal OKF Source document alongside pasted text
//! and URLs. A format we can't parse reliably is rejected with a clear
//! error rather than silently degraded — a source that failed to extract
//! should never enter the wiki looking like it succeeded.
//!
//! Adapters return [`ExtractResult`] with stats, provenance, and quality
//! diagnostics. Ingest persists that metadata on Source frontmatter;
//! [`read_source_file`] still exposes only the text for callers that do
//! not need the structured result.

mod adapters;
mod attempt;
mod contract;
mod fetch;
mod file;
mod limits;
mod quality;

#[cfg(test)]
mod test_support;

#[allow(unused_imports)]
pub use attempt::{
    AttemptChain, AttemptDenied, AttemptRecord, ChainOutcome, DEFAULT_MAX_ATTEMPT_DEPTH,
};
#[allow(unused_imports)]
pub use contract::{
    resolve_quality_outcome, AssetKind, AssetRef, Block, BlockContent, BlockKind, Diagnostic,
    DiagnosticCode, DocumentMeta, ExtractResult, ExtractResultSummary, ExtractStats,
    ExtractionMode, FallbackKind, FallbackTarget, OfficeStats, PdfPageStats, PdfStats, Provenance,
    QualityClass, QualityMetrics, QualityOutcome, QualityReason, QualityReport, SourceFormat,
    SourceLocation, SpreadsheetStats, WebStats, MARKDOWN_SPEC_VERSION, QUALITY_POLICY_VERSION,
};
pub use quality::evaluate_quality;

use std::path::Path;

use adapters::extract_web_page;
use limits::{validate_extracted_text, MAX_EXTRACTED_TEXT_BYTES};

pub const BUILTIN_EXTRACTOR: &str = "builtin";
pub const WEB_EXTRACTOR: &str = "readabilityrs+htmd";
pub const WEB_BODY_EXTRACTOR: &str = "semantic-body+htmd";
pub const WEB_EXTRACTOR_VERSION: &str = "readabilityrs@0.1.4+htmd@0.5";
pub const WEB_BODY_EXTRACTOR_VERSION: &str = "semantic-body@1+htmd@0.5";

/// Binary formats this module knows how to convert to text.
pub const BINARY_EXTENSIONS: &[&str] = &[
    "pdf", "doc", "docx", "docm", "rtf", "epub", "ppt", "pps", "pot", "pptx", "pptm", "ppsx",
    "ppsm", "xlsx", "xls", "xlsm", "xlsb", "ods", "odt", "odp",
];

/// Raster images. Images are OCR'd on import through the OS-native provider
/// (Vision on macOS, Windows.Media.Ocr on Windows); scanned-PDF OCR is not
/// supported yet.
pub const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "tif", "tiff", "bmp"];

/// Local HTML files go through the same static web extract path as URLs.
pub const HTML_EXTENSIONS: &[&str] = &["html", "htm"];

/// Formats that are already text and only need reading, not parsing.
pub const PLAIN_TEXT_EXTENSIONS: &[&str] = &[
    "md", "mdx", "txt", "csv", "tsv", "json", "xml", "yaml", "yml",
];

/// Every extension this module can turn into ingestible text, binary or
/// plain. The file picker and the dispatcher below both read from this so
/// they can never drift.
pub fn all_supported_extensions() -> Vec<&'static str> {
    BINARY_EXTENSIONS
        .iter()
        .chain(IMAGE_EXTENSIONS)
        .chain(HTML_EXTENSIONS)
        .chain(PLAIN_TEXT_EXTENSIONS)
        .copied()
        .collect()
}

pub fn is_supported(path: &Path) -> bool {
    extension_of(path).is_some_and(|extension| {
        BINARY_EXTENSIONS.contains(&extension.as_str())
            || IMAGE_EXTENSIONS.contains(&extension.as_str())
            || HTML_EXTENSIONS.contains(&extension.as_str())
            || PLAIN_TEXT_EXTENSIONS.contains(&extension.as_str())
    })
}

/// Fetch a URL, extract the article to Markdown, and refuse SPA/login shells.
pub fn extract_url(url: &str) -> Result<ExtractResult, String> {
    #[cfg(test)]
    {
        return extract_fetched_page(fetch::fetch_url_with(
            url,
            fetch::FetchPolicy::allow_loopback(),
        )?);
    }
    #[cfg(not(test))]
    extract_fetched_page(fetch::fetch_url(url)?)
}

fn extract_fetched_page(page: fetch::FetchedPage) -> Result<ExtractResult, String> {
    validate_extracted_text(&page.html, MAX_EXTRACTED_TEXT_BYTES)?;
    extract_web_page(page, SourceFormat::Url)
}

/// Read a source file into text. User-visible ingest still depends on this
/// string; structured stats live on [`extract_source`].
pub fn read_source_file(path: &Path) -> Result<String, String> {
    Ok(extract_source(path)?.text)
}

/// Run file adapters with AttemptChain fallbacks (builtin → AnyDoc). Images
/// are OCR'd on import through the OS-native provider.
pub fn extract_source(path: &Path) -> Result<ExtractResult, String> {
    file::extract_source_file(path)
}

fn extension_of(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_support::{write_minimal_odf_zip, write_minimal_pptx, write_minimal_xlsx};

    #[test]
    fn rejects_unsupported_extensions_with_a_clear_message() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("legacy.bin");
        std::fs::write(&path, b"not a known format").unwrap();
        let error = read_source_file(&path).unwrap_err();
        assert!(error.contains("not a supported source format"));
        assert!(!is_supported(&path));
    }

    #[test]
    fn plain_text_formats_are_read_directly() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("notes.txt");
        std::fs::write(&path, "Plain notes.").unwrap();
        assert!(is_supported(&path));
        assert_eq!(read_source_file(&path).unwrap(), "Plain notes.");
    }

    #[test]
    fn xlsx_becomes_a_markdown_table_per_sheet() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("book.xlsx");
        write_minimal_xlsx(&path, "Sheet1", &[["Name", "Score"], ["Ada", "10"]]);
        let text = read_source_file(&path).unwrap();
        assert!(text.contains("## Sheet1"));
        assert!(text.contains("| Name | Score |"));
        assert!(text.contains("| Ada | 10 |"));
    }

    #[test]
    fn pptx_extracts_slide_text_in_order() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("deck.pptx");
        write_minimal_pptx(&path, &["First slide title", "Second slide title"]);
        let text = read_source_file(&path).unwrap();
        let first = text.find("First slide title").unwrap();
        let second = text.find("Second slide title").unwrap();
        assert!(first < second);
        assert!(text.contains("## Slide 1"));
        assert!(text.contains("## Slide 2"));
    }

    #[test]
    fn odt_extracts_paragraph_text() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("note.odt");
        write_minimal_odf_zip(
            &path,
            "<office:document-content xmlns:office=\"urn:office\" xmlns:text=\"urn:text\">\
             <office:body><office:text>\
             <text:p>Hello from an ODF document.</text:p>\
             </office:text></office:body></office:document-content>",
        );
        let text = read_source_file(&path).unwrap();
        assert_eq!(text, "Hello from an ODF document.");
    }

    #[test]
    fn extract_source_keeps_the_same_text_as_read_source_file() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("notes.txt");
        std::fs::write(&path, "  Plain notes.  ").unwrap();
        let result = extract_source(&path).unwrap();
        assert_eq!(result.text, read_source_file(&path).unwrap());
        assert_eq!(result.text, "Plain notes.");
        assert_eq!(result.provenance.extractor, BUILTIN_EXTRACTOR);
        assert_eq!(result.provenance.mode, ExtractionMode::LocalFast);
        assert_eq!(result.document.format, SourceFormat::Text);
        assert!(result.stats.char_count > 0);
    }

    #[test]
    fn spreadsheet_stats_count_sheets_and_cells() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("book.xlsx");
        write_minimal_xlsx(&path, "Sheet1", &[["Name", "Score"], ["Ada", "10"]]);
        let result = extract_source(&path).unwrap();
        assert_eq!(result.text, read_source_file(&path).unwrap());
        let sheets = result.stats.spreadsheet.expect("spreadsheet stats");
        assert_eq!(sheets.sheet_count, 1);
        assert_eq!(sheets.visible_sheet_count, 1);
        assert_eq!(sheets.non_empty_cell_count, 4);
        assert_eq!(result.stats.coverage_bps, Some(10_000));
    }

    #[test]
    fn slide_stats_count_every_slide_part() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("deck.pptx");
        write_minimal_pptx(&path, &["First slide title", "Second slide title"]);
        let result = extract_source(&path).unwrap();
        assert_eq!(result.text, read_source_file(&path).unwrap());
        let office = result.stats.office.expect("office stats");
        assert_eq!(office.slide_count, 2);
        assert_eq!(office.slides_with_text, 2);
        assert_eq!(result.stats.coverage_bps, Some(10_000));
    }

    #[test]
    fn odt_stats_count_paragraphs() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("note.odt");
        write_minimal_odf_zip(
            &path,
            "<office:document-content xmlns:office=\"urn:office\" xmlns:text=\"urn:text\">\
             <office:body><office:text>\
             <text:p>Hello from an ODF document.</text:p>\
             <text:p></text:p>\
             </office:text></office:body></office:document-content>",
        );
        let result = extract_source(&path).unwrap();
        assert_eq!(result.text, "Hello from an ODF document.");
        let office = result.stats.office.expect("office stats");
        assert_eq!(office.paragraph_count, 2);
        assert_eq!(office.non_empty_paragraph_count, 1);
        assert_eq!(result.stats.coverage_bps, Some(5_000));
    }

    #[test]
    fn quality_diagnostics_do_not_change_ingested_text() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("notes.txt");
        let body = format!("{} and some readable notes here", "\u{FFFD}".repeat(5));
        std::fs::write(&path, &body).unwrap();
        let result = extract_source(&path).unwrap();
        assert_eq!(result.text, read_source_file(&path).unwrap());
        assert_eq!(result.text, body);
        assert!(result
            .diagnostics
            .iter()
            .any(|item| item.code == DiagnosticCode::ReplacementCharRatioHigh));
    }

    #[test]
    fn html_files_extract_article_markdown() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("retry.html");
        std::fs::write(
            &path,
            r#"<!doctype html><html><head><title>Retry Patterns</title></head>
<body><nav><a href="/">Home</a></nav>
<article><h1>Retry Patterns</h1>
<p>Exponential backoff retries a failed request after a delay that doubles each time, up to a configured maximum. That keeps a struggling dependency from being stampeded by a herd of identical clients.</p>
<p>Jitter spreads those retries so many clients do not wake up on the same tick. Together with a retry budget, the service stays available for new work.</p>
</article></body></html>"#,
        )
        .unwrap();
        let result = extract_source(&path).unwrap();
        assert!(result.text.contains("Exponential backoff"));
        assert_eq!(result.provenance.extractor, WEB_EXTRACTOR);
        assert_eq!(result.document.format, SourceFormat::Html);
        assert_eq!(result.text, read_source_file(&path).unwrap());
    }

    #[test]
    fn extract_url_uses_the_static_web_path_on_loopback() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let body = r#"<!doctype html><html><head><title>Retry Patterns</title></head>
<body><article><h1>Retry Patterns</h1>
<p>Exponential backoff retries a failed request after a delay that doubles each time, up to a configured maximum. That keeps a struggling dependency from being stampeded by a herd of identical clients.</p>
<p>Jitter spreads those retries so many clients do not wake up on the same tick. Together with a retry budget, the service stays available for new work.</p>
</article></body></html>"#;
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        });
        let url = format!("http://127.0.0.1:{}/", addr.port());
        let page = super::fetch::fetch_url_with(&url, super::fetch::FetchPolicy::allow_loopback())
            .unwrap();
        let result = extract_fetched_page(page).unwrap();
        assert!(result.text.contains("Exponential backoff"));
        assert_eq!(result.document.format, SourceFormat::Url);
        assert_eq!(result.provenance.extractor, WEB_EXTRACTOR);
        assert_eq!(
            result
                .stats
                .web
                .as_ref()
                .and_then(|web| web.final_url.as_deref()),
            Some(url.as_str())
        );
    }
}
