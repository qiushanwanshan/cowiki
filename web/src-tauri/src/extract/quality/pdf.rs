//! PDF page-level completeness and glyph quality.

use super::policy::{
    PDF_INVALID_GLYPH_RATIO_LIMIT, PDF_MIN_VALID_CHARS_PER_PAGE, PDF_TEXTLESS_PAGE_RATIO_LIMIT,
};
use crate::extract::contract::{
    DiagnosticCode, ExtractResult, QualityClass, QualityReason, SourceLocation,
};

pub fn collect(result: &ExtractResult, reasons: &mut Vec<QualityReason>) {
    let Some(pdf) = result.stats.pdf.as_ref() else {
        return;
    };
    if pdf.page_count == 0 {
        return;
    }

    let textless_pages = pdf
        .pages
        .iter()
        .filter(|page| page.valid_chars < PDF_MIN_VALID_CHARS_PER_PAGE)
        .count() as u32;
    let textless_ratio = textless_pages as f32 / pdf.page_count as f32;

    for page in &pdf.pages {
        if page.valid_chars < PDF_MIN_VALID_CHARS_PER_PAGE {
            reasons.push(
                QualityReason::new(
                    DiagnosticCode::PdfTextlessPage,
                    QualityClass::StructureWarn,
                    format!(
                        "page {} has only {} valid characters",
                        page.page, page.valid_chars
                    ),
                )
                .at(SourceLocation::page(page.page)),
            );
        }
        if page.invalid_glyph_ratio > PDF_INVALID_GLYPH_RATIO_LIMIT {
            reasons.push(
                QualityReason::new(
                    DiagnosticCode::PdfInvalidGlyphRatioHigh,
                    QualityClass::StructureWarn,
                    format!(
                        "page {} invalid-glyph ratio is {:.1}%",
                        page.page,
                        page.invalid_glyph_ratio * 100.0
                    ),
                )
                .at(SourceLocation::page(page.page)),
            );
        }
    }

    if textless_ratio >= PDF_TEXTLESS_PAGE_RATIO_LIMIT {
        let (code, message) = if pdf.pages_with_valid_text == 0 {
            (
                DiagnosticCode::PdfScannedDocument,
                format!(
                    "none of {textless_pages}/{} pages have a usable text layer",
                    pdf.page_count
                ),
            )
        } else {
            (
                DiagnosticCode::PdfMixedDocument,
                format!(
                    "{textless_pages}/{} pages have no usable text layer ({:.0}%)",
                    pdf.page_count,
                    textless_ratio * 100.0
                ),
            )
        };
        reasons.push(QualityReason::new(
            code,
            QualityClass::Completeness,
            message,
        ));
    }
}
