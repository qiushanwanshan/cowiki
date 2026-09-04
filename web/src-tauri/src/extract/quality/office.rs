//! Office and spreadsheet completeness: paragraphs, slides, sheets, tables.

use super::policy::{COVERAGE_FALLBACK_BPS, COVERAGE_FULL_BPS};
use crate::extract::contract::{
    DiagnosticCode, ExtractResult, QualityClass, QualityReason, SourceFormat,
};

pub fn collect(result: &ExtractResult, reasons: &mut Vec<QualityReason>) {
    match result.document.format {
        SourceFormat::Docx | SourceFormat::Odt => collect_document(result, reasons),
        SourceFormat::Pptx | SourceFormat::Odp => collect_slides(result, reasons),
        SourceFormat::Xlsx | SourceFormat::Xls | SourceFormat::Ods => {
            collect_spreadsheet(result, reasons)
        }
        _ => {}
    }
}

fn collect_document(result: &ExtractResult, reasons: &mut Vec<QualityReason>) {
    let Some(office) = result.stats.office.as_ref() else {
        return;
    };
    push_coverage(
        reasons,
        result.stats.paragraph_count as u32,
        office.non_empty_paragraph_count,
        DiagnosticCode::DocxParagraphCoverageLow,
        "non-empty paragraph",
    );
    if office.table_count > 0 && result.stats.table_count == 0 {
        reasons.push(QualityReason::new(
            DiagnosticCode::DocxTableMissing,
            QualityClass::Completeness,
            format!(
                "source has {} table(s) but the output has none",
                office.table_count
            ),
        ));
    }
}

fn collect_slides(result: &ExtractResult, reasons: &mut Vec<QualityReason>) {
    let Some(office) = result.stats.office.as_ref() else {
        return;
    };
    push_coverage(
        reasons,
        office.slides_with_text,
        office.slide_count,
        DiagnosticCode::PptxSlideMissing,
        "slide",
    );
}

fn collect_spreadsheet(result: &ExtractResult, reasons: &mut Vec<QualityReason>) {
    let Some(sheets) = result.stats.spreadsheet.as_ref() else {
        return;
    };
    if sheets.non_empty_cell_count > 0 && result.stats.table_count == 0 {
        reasons.push(QualityReason::new(
            DiagnosticCode::XlsxSheetMissing,
            QualityClass::Completeness,
            format!(
                "workbook has {} non-empty cell(s) across {} sheet(s) but no sheet was rendered",
                sheets.non_empty_cell_count, sheets.sheet_count
            ),
        ));
    }
}

fn push_coverage(
    reasons: &mut Vec<QualityReason>,
    covered: u32,
    total: u32,
    code: DiagnosticCode,
    unit: &str,
) {
    if total == 0 {
        return;
    }
    let bps = ((u64::from(covered) * 10_000) / u64::from(total)) as u32;
    if bps >= COVERAGE_FULL_BPS {
        return;
    }
    let class = if bps < COVERAGE_FALLBACK_BPS {
        QualityClass::Completeness
    } else {
        QualityClass::StructureWarn
    };
    reasons.push(QualityReason::new(
        code,
        class,
        format!(
            "{covered}/{total} {unit}(s) produced output ({:.0}%)",
            bps as f32 / 100.0
        ),
    ));
}
