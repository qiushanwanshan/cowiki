//! Format-agnostic quality rules: emptiness, garbled text, repetition, stubs.

use super::policy::{
    CONTROL_CHAR_RATIO_LIMIT, OUTPUT_TOO_SHORT_CHARS, REPEATED_BLOCK_MIN_CHARS, REPEATED_BLOCK_RUN,
    REPLACEMENT_CHAR_RATIO_LIMIT, SUBSTANTIAL_CELLS, SUBSTANTIAL_PARAGRAPHS, SUBSTANTIAL_PDF_PAGES,
    SUBSTANTIAL_SHEETS, SUBSTANTIAL_SLIDES,
};
use crate::extract::contract::{
    DiagnosticCode, ExtractResult, ExtractStats, QualityClass, QualityReason,
};

pub fn collect(result: &ExtractResult, reasons: &mut Vec<QualityReason>) {
    if result.text.trim().is_empty() {
        reasons.push(QualityReason::new(
            DiagnosticCode::EmptyOutput,
            QualityClass::HardFail,
            "extracted text is empty after trimming whitespace",
        ));
        return;
    }

    let stats = &result.stats;
    if stats.replacement_char_ratio() > REPLACEMENT_CHAR_RATIO_LIMIT {
        reasons.push(QualityReason::new(
            DiagnosticCode::ReplacementCharRatioHigh,
            QualityClass::Readability,
            format!(
                "replacement characters are {:.1}% of output (limit {:.0}%)",
                stats.replacement_char_ratio() * 100.0,
                REPLACEMENT_CHAR_RATIO_LIMIT * 100.0
            ),
        ));
    }
    if stats.control_char_ratio() > CONTROL_CHAR_RATIO_LIMIT {
        reasons.push(QualityReason::new(
            DiagnosticCode::ControlCharRatioHigh,
            QualityClass::Readability,
            format!(
                "illegal control characters are {:.1}% of output (limit {:.0}%)",
                stats.control_char_ratio() * 100.0,
                CONTROL_CHAR_RATIO_LIMIT * 100.0
            ),
        ));
    }
    if looks_like_template(&result.text) {
        reasons.push(QualityReason::new(
            DiagnosticCode::TemplateOutput,
            QualityClass::Readability,
            "output looks like a URL, filename, or parser stub rather than document text",
        ));
    }
    if has_repeated_blocks(&result.text) {
        reasons.push(QualityReason::new(
            DiagnosticCode::RepeatedContent,
            QualityClass::Readability,
            "the same block of text repeats three or more times in a row",
        ));
    }
    if stats.char_count < OUTPUT_TOO_SHORT_CHARS && source_looks_substantial(stats) {
        reasons.push(QualityReason::new(
            DiagnosticCode::OutputTooShort,
            QualityClass::Readability,
            format!(
                "output is only {} characters while the source has substantial structure",
                stats.char_count
            ),
        ));
    }
}

fn looks_like_template(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    trimmed.lines().all(|line| {
        let line = line.trim();
        line.is_empty() || is_bare_url(line) || is_bare_filename(line)
    })
}

fn is_bare_url(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    !text.contains(char::is_whitespace)
        && (lower.starts_with("http://")
            || lower.starts_with("https://")
            || lower.starts_with("file:"))
}

fn is_bare_filename(text: &str) -> bool {
    !text.contains(char::is_whitespace)
        && text.contains('.')
        && text
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | '/'))
        && text.len() < 80
}

fn has_repeated_blocks(text: &str) -> bool {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| line.chars().count() >= REPEATED_BLOCK_MIN_CHARS)
        .collect();
    if lines.len() < REPEATED_BLOCK_RUN {
        return false;
    }
    let mut run = 1;
    for pair in lines.windows(2) {
        if pair[0] == pair[1] {
            run += 1;
            if run >= REPEATED_BLOCK_RUN {
                return true;
            }
        } else {
            run = 1;
        }
    }
    false
}

fn source_looks_substantial(stats: &ExtractStats) -> bool {
    stats
        .pdf
        .as_ref()
        .is_some_and(|pdf| pdf.page_count >= SUBSTANTIAL_PDF_PAGES)
        || stats.office.as_ref().is_some_and(|office| {
            office.paragraph_count >= SUBSTANTIAL_PARAGRAPHS
                || office.slide_count >= SUBSTANTIAL_SLIDES
        })
        || stats.spreadsheet.as_ref().is_some_and(|sheets| {
            sheets.sheet_count >= SUBSTANTIAL_SHEETS
                || sheets.non_empty_cell_count >= SUBSTANTIAL_CELLS
        })
        || stats.web.as_ref().is_some_and(|web| {
            web.response_bytes >= super::policy::WEB_LARGE_HTML_BYTES
                || web.dom_visible_text_chars >= 500
        })
}
