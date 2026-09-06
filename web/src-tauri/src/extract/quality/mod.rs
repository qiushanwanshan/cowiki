//! Format-specific quality gates. Outcomes are decided by reason class,
//! not by a single score, and never by output length.

mod common;
mod office;
mod pdf;
mod policy;
mod web;

pub(crate) use policy::WEB_MIN_ARTICLE_CHARS;

#[cfg(test)]
mod tests;

use crate::extract::contract::{
    Diagnostic, DiagnosticCode, ExtractResult, FallbackKind, FallbackTarget, QualityClass,
    QualityMetrics, QualityReason, QualityReport, SourceFormat,
};

/// Evaluate stats and text against the first-round rules. Callers still
/// decide whether to ingest; this function does not write Source files.
pub fn evaluate_quality(result: &ExtractResult) -> QualityReport {
    let mut reasons = Vec::new();
    common::collect(result, &mut reasons);
    pdf::collect(result, &mut reasons);
    office::collect(result, &mut reasons);
    web::collect(result, &mut reasons);

    let suggested_fallback = suggested_fallback(result.document.format, &reasons);
    let warning_count = reasons
        .iter()
        .filter(|reason| reason.class == QualityClass::StructureWarn)
        .count();
    QualityReport::from_reasons(
        reasons,
        QualityMetrics::from_stats(&result.stats, warning_count),
        suggested_fallback,
    )
}

/// Copy quality reasons onto `result.diagnostics` without changing `text`.
pub fn attach_diagnostics(result: &mut ExtractResult, report: &QualityReport) {
    result.diagnostics = report
        .reasons
        .iter()
        .map(|reason| {
            let diagnostic = Diagnostic::new(reason.code, reason.message.clone());
            match &reason.location {
                Some(location) => diagnostic.at(location.clone()),
                None => diagnostic,
            }
        })
        .collect();
}

fn suggested_fallback(format: SourceFormat, reasons: &[QualityReason]) -> Option<FallbackTarget> {
    if reasons
        .iter()
        .any(|reason| reason.class == QualityClass::HardFail)
    {
        return None;
    }
    let needs_recovery = reasons.iter().any(|reason| {
        matches!(
            reason.class,
            QualityClass::Completeness | QualityClass::Readability
        ) || matches!(
            reason.code,
            DiagnosticCode::PdfTextlessPage
                | DiagnosticCode::PdfInvalidGlyphRatioHigh
                | DiagnosticCode::PdfMixedDocument
                | DiagnosticCode::PdfScannedDocument
        )
    });
    if !needs_recovery {
        return None;
    }
    match format {
        SourceFormat::Pdf | SourceFormat::Image => {
            Some(FallbackTarget::capability(FallbackKind::PageOcr, "ocr"))
        }
        SourceFormat::Docx
        | SourceFormat::Odt
        | SourceFormat::Pptx
        | SourceFormat::Odp
        | SourceFormat::Xlsx
        | SourceFormat::Xls
        | SourceFormat::Ods => Some(FallbackTarget::local(FallbackKind::AnyDoc)),
        SourceFormat::Html | SourceFormat::Url => {
            if reasons
                .iter()
                .any(|reason| reason.code == DiagnosticCode::WebSpaShellDetected)
            {
                Some(FallbackTarget::local(FallbackKind::IsolatedBrowser))
            } else {
                Some(FallbackTarget::local(FallbackKind::SemanticBody))
            }
        }
        _ => None,
    }
}
