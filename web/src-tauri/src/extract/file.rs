//! File extraction pipeline with AttemptChain fallbacks.

use std::path::Path;
use std::time::Instant;

use super::adapters::{
    extract_anydoc, extract_docx, extract_ocr_image_result, extract_pdf, extract_slides,
    extract_spreadsheet, extract_zip_xml_part, is_anydoc_primary, AdapterOutput,
    ANYDOC_EXTRACTOR, ANYDOC_SETTINGS_HASH,
};
use super::attempt::{AttemptChain, AttemptRecord};
use super::contract::{
    DiagnosticCode, ExtractResult, FallbackKind, Provenance, QualityClass, QualityMetrics,
    QualityReason, QualityReport, SourceFormat,
};
use super::limits::{
    validate_archive_limits, validate_extracted_text, validate_file_size, MAX_ARCHIVE_ENTRIES,
    MAX_ARCHIVE_ENTRY_BYTES, MAX_ARCHIVE_TOTAL_BYTES, MAX_EXTRACTED_TEXT_BYTES,
    MAX_SOURCE_FILE_BYTES,
};
use super::quality::{attach_diagnostics, evaluate_quality};
use super::BUILTIN_EXTRACTOR;
use super::{HTML_EXTENSIONS, IMAGE_EXTENSIONS, PLAIN_TEXT_EXTENSIONS};

pub fn extract_source_file(path: &Path) -> Result<ExtractResult, String> {
    let extension = extension_of(path)
        .ok_or_else(|| "file has no extension to identify its format".to_string())?;
    validate_file_size(path, MAX_SOURCE_FILE_BYTES)?;
    if HTML_EXTENSIONS.contains(&extension.as_str()) {
        let html =
            std::fs::read_to_string(path).map_err(|error| format!("cannot read file: {error}"))?;
        validate_extracted_text(&html, MAX_EXTRACTED_TEXT_BYTES)?;
        return super::adapters::extract_html_file(&html);
    }

    let format = SourceFormat::from_extension(&extension);
    if IMAGE_EXTENSIONS.contains(&extension.as_str()) {
        return extract_image(path, format);
    }

    validate_archive_if_needed(path, &extension)?;
    let mut chain = AttemptChain::new();

    if is_anydoc_primary(&extension) {
        try_anydoc(&mut chain, path, format)?;
    } else {
        try_builtin(&mut chain, path, &extension, format)?;
    }

    run_fallbacks(&mut chain, path, format)?;
    finish_file_chain(chain)
}

fn extract_image(path: &Path, _format: SourceFormat) -> Result<ExtractResult, String> {
    // OCR is part of image ingestion and uses the OS-native provider (Vision
    // on macOS, Windows.Media.Ocr on Windows) so users never have to install
    // an OCR dependency themselves.
    extract_ocr_image_result(path)
}

fn run_fallbacks(
    chain: &mut AttemptChain,
    path: &Path,
    format: SourceFormat,
) -> Result<(), String> {
    loop {
        let Some(last) = chain.attempts().last() else {
            break;
        };
        if last.quality.outcome.is_usable() {
            break;
        }
        let fallback = last
            .quality
            .suggested_fallback
            .as_ref()
            .map(|target| target.kind);
        let tried_anydoc = chain
            .attempts()
            .iter()
            .any(|attempt| attempt.extractor == ANYDOC_EXTRACTOR);

        match fallback {
            Some(FallbackKind::AnyDoc) if !tried_anydoc => {
                try_anydoc(chain, path, format)?;
                continue;
            }
            // PageOcr currently has no provider: scanned-PDF OCR is not
            // supported yet, so the chain ends here and the caller gets the
            // pending_ocr message from the quality report.
            _ => break,
        }
    }
    Ok(())
}

fn try_builtin(
    chain: &mut AttemptChain,
    path: &Path,
    extension: &str,
    format: SourceFormat,
) -> Result<(), String> {
    let started = Instant::now();
    let output = if PLAIN_TEXT_EXTENSIONS.contains(&extension) {
        let text =
            std::fs::read_to_string(path).map_err(|error| format!("cannot read file: {error}"))?;
        AdapterOutput::from_text(text)
    } else {
        match extension {
            "pdf" => extract_pdf(path)?,
            "docx" => extract_docx(path)?,
            "xlsx" | "xls" | "ods" => extract_spreadsheet(path)?,
            "pptx" => extract_slides(path)?,
            "odt" | "odp" => extract_zip_xml_part(path, "content.xml")?,
            other => {
                return Err(format!("'.{other}' is not a supported source format"));
            }
        }
    };
    let trimmed = output.text.trim();
    if trimmed.is_empty() {
        if format == SourceFormat::Pdf {
            return Err(
                "scanned PDF needs OCR; PDF OCR is not supported yet (image files are supported)"
                    .to_string(),
            );
        }
        return Err("no extractable text found in this file".to_string());
    }
    let result = ExtractResult::from_adapter(
        trimmed.to_string(),
        format,
        output.stats,
        Provenance::local_fast(
            BUILTIN_EXTRACTOR,
            env!("CARGO_PKG_VERSION"),
            started.elapsed().as_millis() as u64,
        ),
    );
    record_attempt(chain, result)
}

fn try_anydoc(chain: &mut AttemptChain, path: &Path, format: SourceFormat) -> Result<(), String> {
    if chain
        .can_attempt(ANYDOC_EXTRACTOR, ANYDOC_SETTINGS_HASH)
        .is_err()
    {
        return Ok(());
    }
    match extract_anydoc(path, format) {
        Ok(result) => record_attempt(chain, result),
        Err(error) => record_failed(chain, ANYDOC_EXTRACTOR, ANYDOC_SETTINGS_HASH, format, error),
    }
}

fn record_attempt(chain: &mut AttemptChain, mut result: ExtractResult) -> Result<(), String> {
    validate_extracted_text(&result.text, MAX_EXTRACTED_TEXT_BYTES)?;
    if result.text.trim().is_empty() {
        return Err("no extractable text found in this file".to_string());
    }
    let report = evaluate_quality(&result);
    attach_diagnostics(&mut result, &report);
    chain
        .record(AttemptRecord::from_result(result, report))
        .map_err(|error| error.to_string())
}

fn record_failed(
    chain: &mut AttemptChain,
    extractor: &str,
    settings_hash: &str,
    format: SourceFormat,
    message: String,
) -> Result<(), String> {
    if chain.can_attempt(extractor, settings_hash).is_err() {
        return Ok(());
    }
    let mut provenance = Provenance::local_fast(extractor, "failed", 0);
    provenance.settings_hash = settings_hash.to_string();
    let result = ExtractResult::from_text(String::new(), format, provenance);
    let report = QualityReport::from_reasons(
        vec![QualityReason::new(
            DiagnosticCode::ParserCrash,
            QualityClass::HardFail,
            message,
        )],
        QualityMetrics::default(),
        None,
    );
    let _ = chain.record(AttemptRecord::from_result(result, report));
    Ok(())
}

fn finish_file_chain(chain: AttemptChain) -> Result<ExtractResult, String> {
    let outcome = chain.finish();
    if let Some(selected) = outcome.selected() {
        let mut result = selected.result.clone();
        attach_diagnostics(&mut result, &selected.quality);
        return Ok(result);
    }
    if let Some(last) = outcome
        .attempts
        .iter()
        .rev()
        .find(|attempt| !attempt.result.text.trim().is_empty())
    {
        let mut result = last.result.clone();
        attach_diagnostics(&mut result, &last.quality);
        return Ok(result);
    }
    let last = outcome
        .attempts
        .last()
        .ok_or_else(|| "no extraction attempts were recorded".to_string())?;
    Err(unusable_file_message(&last.quality))
}

fn unusable_file_message(report: &QualityReport) -> String {
    if report
        .suggested_fallback
        .as_ref()
        .is_some_and(|target| target.kind == FallbackKind::PageOcr)
    {
        return "this file needs OCR; it was imported and marked pending_ocr".to_string();
    }
    let codes: Vec<_> = report
        .reasons
        .iter()
        .map(|reason| reason.code.as_str())
        .collect();
    if codes.is_empty() {
        "could not extract reliable text from this file".to_string()
    } else {
        format!(
            "could not extract reliable text from this file ({})",
            codes.join(", ")
        )
    }
}

fn validate_archive_if_needed(path: &Path, extension: &str) -> Result<(), String> {
    if matches!(
        extension,
        "docx" | "xlsx" | "ods" | "pptx" | "odt" | "odp" | "docm" | "pptm" | "xlsm"
    ) {
        validate_archive_limits(
            path,
            MAX_ARCHIVE_ENTRY_BYTES,
            MAX_ARCHIVE_TOTAL_BYTES,
            MAX_ARCHIVE_ENTRIES,
        )?;
    }
    Ok(())
}

fn extension_of(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anydoc_primary_format_skips_builtin() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("note.rtf");
        std::fs::write(
            &path,
            r"{\rtf1\ansi\deff0{\fonttbl{\f0 Helvetica;}}\f0\fs24 AnyDoc primary RTF.}",
        )
        .unwrap();
        let result = extract_source_file(&path).unwrap();
        assert!(result.text.contains("AnyDoc primary RTF"));
        assert_eq!(result.provenance.extractor, ANYDOC_EXTRACTOR);
    }
}
