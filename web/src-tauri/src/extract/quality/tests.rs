use super::evaluate_quality;
use crate::extract::contract::{
    DiagnosticCode, ExtractResult, ExtractStats, ExtractionMode, FallbackKind, OfficeStats,
    PdfPageStats, PdfStats, Provenance, QualityClass, QualityOutcome, SourceFormat, SourceLocation,
    SpreadsheetStats, WebStats,
};

fn sample(format: SourceFormat, text: &str, mut stats: ExtractStats) -> ExtractResult {
    stats.apply_text_counts(text);
    ExtractResult::from_adapter(
        text,
        format,
        stats,
        Provenance::local_fast("builtin", "test", 1),
    )
}

fn pdf_pages(pages: &[(u32, f32)]) -> ExtractStats {
    let page_stats: Vec<PdfPageStats> = pages
        .iter()
        .enumerate()
        .map(|(index, (valid_chars, invalid_glyph_ratio))| PdfPageStats {
            page: (index + 1) as u32,
            valid_chars: *valid_chars,
            invalid_glyph_ratio: *invalid_glyph_ratio,
            image_coverage: 0.0,
        })
        .collect();
    let pages_with_valid_text = page_stats
        .iter()
        .filter(|page| page.valid_chars >= 20)
        .count() as u32;
    ExtractStats {
        pdf: Some(PdfStats {
            page_count: page_stats.len() as u32,
            pages_with_valid_text,
            pages: page_stats,
        }),
        ..ExtractStats::default()
    }
}

#[test]
fn clean_plain_text_passes() {
    let result = sample(
        SourceFormat::Text,
        "A readable paragraph of notes.",
        ExtractStats::default(),
    );
    let report = evaluate_quality(&result);
    assert_eq!(report.outcome, QualityOutcome::Pass);
    assert!(report.reasons.is_empty());
    assert!(report.suggested_fallback.is_none());
}

#[test]
fn empty_output_is_a_hard_fail() {
    let report = evaluate_quality(&sample(SourceFormat::Text, "   ", ExtractStats::default()));
    assert_eq!(report.outcome, QualityOutcome::Fail);
    assert_eq!(report.reasons[0].code, DiagnosticCode::EmptyOutput);
    assert_eq!(report.reasons[0].class, QualityClass::HardFail);
    assert!(report.suggested_fallback.is_none());
}

#[test]
fn replacement_chars_at_one_percent_still_pass() {
    let text = format!("{}\u{FFFD}", "a".repeat(99));
    let report = evaluate_quality(&sample(SourceFormat::Text, &text, ExtractStats::default()));
    assert_eq!(report.outcome, QualityOutcome::Pass);
}

#[test]
fn replacement_chars_above_one_percent_fail_for_plain_text() {
    let text = format!("{}\u{FFFD}\u{FFFD}", "a".repeat(98));
    let report = evaluate_quality(&sample(SourceFormat::Text, &text, ExtractStats::default()));
    assert_eq!(report.outcome, QualityOutcome::Fail);
    assert_eq!(
        report.reasons[0].code,
        DiagnosticCode::ReplacementCharRatioHigh
    );
    assert!(report.suggested_fallback.is_none());
}

#[test]
fn replacement_chars_on_a_pdf_suggest_page_ocr() {
    let text = format!("{}\u{FFFD}\u{FFFD}", "a".repeat(98));
    let report = evaluate_quality(&sample(SourceFormat::Pdf, &text, pdf_pages(&[(100, 0.02)])));
    assert_eq!(report.outcome, QualityOutcome::Fallback);
    assert_eq!(
        report.suggested_fallback.as_ref().unwrap().kind,
        FallbackKind::PageOcr
    );
}

#[test]
fn control_chars_above_one_percent_are_readability_failures() {
    let text = format!("{}{}", "\u{0001}".repeat(2), "a".repeat(98));
    let report = evaluate_quality(&sample(SourceFormat::Text, &text, ExtractStats::default()));
    assert_eq!(report.outcome, QualityOutcome::Fail);
    assert_eq!(report.reasons[0].code, DiagnosticCode::ControlCharRatioHigh);
}

#[test]
fn url_only_output_is_treated_as_template_text() {
    let report = evaluate_quality(&sample(
        SourceFormat::Text,
        "https://example.com/article",
        ExtractStats::default(),
    ));
    assert_eq!(report.outcome, QualityOutcome::Fail);
    assert_eq!(report.reasons[0].code, DiagnosticCode::TemplateOutput);
}

#[test]
fn three_repeated_blocks_trigger_readability_fallback_on_pdf() {
    let block = "This paragraph is long enough to count as a repeated block.";
    let text = [block, block, block].join("\n");
    let report = evaluate_quality(&sample(SourceFormat::Pdf, &text, pdf_pages(&[(80, 0.0)])));
    assert_eq!(report.outcome, QualityOutcome::Fallback);
    assert!(report
        .reasons
        .iter()
        .any(|reason| reason.code == DiagnosticCode::RepeatedContent));
}

#[test]
fn short_output_against_a_multi_page_pdf_is_not_pass() {
    let mut stats = pdf_pages(&[(5, 0.0), (5, 0.0), (5, 0.0)]);
    stats.char_count = 12;
    let report = evaluate_quality(&sample(SourceFormat::Pdf, "tiny stub", stats));
    assert_eq!(report.outcome, QualityOutcome::Fallback);
    assert!(report
        .reasons
        .iter()
        .any(|reason| reason.code == DiagnosticCode::OutputTooShort));
}

#[test]
fn a_few_textless_pdf_pages_warn_and_suggest_page_ocr() {
    let stats = pdf_pages(&[(80, 0.0), (4, 0.0), (90, 0.0), (85, 0.0), (88, 0.0)]);
    let report = evaluate_quality(&sample(
        SourceFormat::Pdf,
        "Enough native text remains on the other pages.",
        stats,
    ));
    assert_eq!(report.outcome, QualityOutcome::Warn);
    assert_eq!(report.reasons[0].code, DiagnosticCode::PdfTextlessPage);
    assert_eq!(report.reasons[0].location, Some(SourceLocation::page(2)));
    assert_eq!(
        report.suggested_fallback.as_ref().unwrap().kind,
        FallbackKind::PageOcr
    );
}

#[test]
fn textless_pages_at_thirty_percent_mark_a_mixed_pdf() {
    let stats = pdf_pages(&[
        (80, 0.0),
        (4, 0.0),
        (90, 0.0),
        (3, 0.0),
        (85, 0.0),
        (88, 0.0),
        (70, 0.0),
        (2, 0.0),
        (60, 0.0),
        (75, 0.0),
    ]);
    let report = evaluate_quality(&sample(
        SourceFormat::Pdf,
        "Two pages still have a usable native text layer.",
        stats,
    ));
    assert_eq!(report.outcome, QualityOutcome::Fallback);
    assert!(report
        .reasons
        .iter()
        .any(|reason| reason.code == DiagnosticCode::PdfMixedDocument));
}

#[test]
fn a_pdf_with_no_valid_text_pages_is_scanned() {
    let stats = pdf_pages(&[(0, 0.0), (3, 0.0), (8, 0.0)]);
    let report = evaluate_quality(&sample(SourceFormat::Pdf, "header", stats));
    assert_eq!(report.outcome, QualityOutcome::Fallback);
    assert!(report
        .reasons
        .iter()
        .any(|reason| reason.code == DiagnosticCode::PdfScannedDocument));
}

#[test]
fn isolated_invalid_glyphs_warn_without_failing_the_document() {
    let stats = pdf_pages(&[(80, 0.0), (80, 0.05), (90, 0.0)]);
    let report = evaluate_quality(&sample(
        SourceFormat::Pdf,
        "Most pages still have a clean native text layer.",
        stats,
    ));
    assert_eq!(report.outcome, QualityOutcome::Warn);
    assert_eq!(
        report.reasons[0].code,
        DiagnosticCode::PdfInvalidGlyphRatioHigh
    );
    assert_eq!(report.reasons[0].location, Some(SourceLocation::page(2)));
}

#[test]
fn low_docx_paragraph_coverage_falls_back_to_anydoc() {
    let stats = ExtractStats {
        paragraph_count: 3,
        office: Some(OfficeStats {
            paragraph_count: 10,
            non_empty_paragraph_count: 10,
            ..OfficeStats::default()
        }),
        ..ExtractStats::default()
    };
    let report = evaluate_quality(&sample(
        SourceFormat::Docx,
        "Only a few of the source paragraphs survived extraction.",
        stats,
    ));
    assert_eq!(report.outcome, QualityOutcome::Fallback);
    assert_eq!(
        report.reasons[0].code,
        DiagnosticCode::DocxParagraphCoverageLow
    );
    assert_eq!(
        report.suggested_fallback.as_ref().unwrap().kind,
        FallbackKind::AnyDoc
    );
}

#[test]
fn modest_docx_paragraph_loss_is_a_warning() {
    let stats = ExtractStats {
        paragraph_count: 8,
        office: Some(OfficeStats {
            non_empty_paragraph_count: 10,
            ..OfficeStats::default()
        }),
        ..ExtractStats::default()
    };
    let report = evaluate_quality(&sample(
        SourceFormat::Docx,
        "Most paragraphs survived, but two were dropped.",
        stats,
    ));
    assert_eq!(report.outcome, QualityOutcome::Warn);
    assert_eq!(report.reasons[0].class, QualityClass::StructureWarn);
}

#[test]
fn missing_docx_tables_are_completeness_failures() {
    let stats = ExtractStats {
        paragraph_count: 4,
        table_count: 0,
        office: Some(OfficeStats {
            non_empty_paragraph_count: 4,
            table_count: 2,
            ..OfficeStats::default()
        }),
        ..ExtractStats::default()
    };
    let report = evaluate_quality(&sample(
        SourceFormat::Docx,
        "Paragraphs remain but every table disappeared.",
        stats,
    ));
    assert_eq!(report.outcome, QualityOutcome::Fallback);
    assert_eq!(report.reasons[0].code, DiagnosticCode::DocxTableMissing);
}

#[test]
fn missing_slides_fall_back_to_anydoc() {
    let stats = ExtractStats {
        office: Some(OfficeStats {
            slide_count: 5,
            slides_with_text: 2,
            ..OfficeStats::default()
        }),
        ..ExtractStats::default()
    };
    let report = evaluate_quality(&sample(
        SourceFormat::Pptx,
        "Only two of the five slides produced any visible text after extraction.",
        stats,
    ));
    assert_eq!(report.outcome, QualityOutcome::Fallback);
    assert_eq!(report.reasons[0].code, DiagnosticCode::PptxSlideMissing);
}

#[test]
fn spreadsheet_with_cells_but_no_rendered_sheet_is_not_pass() {
    let stats = ExtractStats {
        table_count: 0,
        spreadsheet: Some(SpreadsheetStats {
            sheet_count: 2,
            visible_sheet_count: 2,
            non_empty_cell_count: 40,
            formula_count: 0,
        }),
        ..ExtractStats::default()
    };
    let report = evaluate_quality(&sample(
        SourceFormat::Xlsx,
        "The workbook still has dozens of populated cells, but no sheet became a table.",
        stats,
    ));
    assert_eq!(report.outcome, QualityOutcome::Fallback);
    assert_eq!(report.reasons[0].code, DiagnosticCode::XlsxSheetMissing);
}

#[test]
fn completeness_outranks_structure_warnings() {
    let stats = ExtractStats {
        paragraph_count: 2,
        office: Some(OfficeStats {
            non_empty_paragraph_count: 10,
            table_count: 0,
            ..OfficeStats::default()
        }),
        ..ExtractStats::default()
    };
    let report = evaluate_quality(&sample(
        SourceFormat::Docx,
        "Too few paragraphs remain after extraction.",
        stats,
    ));
    assert_eq!(report.outcome, QualityOutcome::Fallback);
    assert!(report
        .reasons
        .iter()
        .any(|reason| reason.class == QualityClass::Completeness));
}

#[test]
fn provenance_mode_is_unchanged_by_the_gate() {
    let result = sample(
        SourceFormat::Text,
        "Notes stay local-fast.",
        ExtractStats::default(),
    );
    let _ = evaluate_quality(&result);
    assert_eq!(result.provenance.mode, ExtractionMode::LocalFast);
}

fn web_stats() -> WebStats {
    WebStats {
        http_status: Some(200),
        final_url: None,
        response_bytes: 12_000,
        article_text_chars: 80,
        article_to_body_text_ratio: 0.04,
        link_text_ratio: 0.1,
        readability_succeeded: false,
        dom_visible_text_chars: 2_000,
        spa_shell_detected: false,
        login_or_challenge_detected: false,
    }
}

fn web_sample(text: &str, web: WebStats) -> ExtractResult {
    sample(
        SourceFormat::Html,
        text,
        ExtractStats {
            web: Some(web),
            ..ExtractStats::default()
        },
    )
}

#[test]
fn web_readability_empty_falls_back_to_semantic_body() {
    let report = evaluate_quality(&web_sample("too short", web_stats()));
    assert_eq!(report.outcome, QualityOutcome::Fallback);
    assert!(report
        .reasons
        .iter()
        .any(|reason| reason.code == DiagnosticCode::WebReadabilityEmpty));
    assert_eq!(
        report.suggested_fallback.as_ref().unwrap().kind,
        FallbackKind::SemanticBody
    );
}

#[test]
fn spa_shell_suggests_isolated_browser() {
    let report = evaluate_quality(&web_sample(
        "You need to enable JavaScript.",
        WebStats {
            spa_shell_detected: true,
            article_text_chars: 12,
            dom_visible_text_chars: 40,
            response_bytes: 8_000,
            ..web_stats()
        },
    ));
    assert_eq!(report.outcome, QualityOutcome::Fallback);
    assert_eq!(
        report.suggested_fallback.as_ref().unwrap().kind,
        FallbackKind::IsolatedBrowser
    );
    assert!(report
        .reasons
        .iter()
        .any(|reason| reason.code == DiagnosticCode::WebSpaShellDetected));
}

#[test]
fn login_wall_is_a_hard_fail() {
    let report = evaluate_quality(&web_sample(
        "Sign in to continue reading this article.",
        WebStats {
            login_or_challenge_detected: true,
            ..web_stats()
        },
    ));
    assert_eq!(report.outcome, QualityOutcome::Fail);
    assert_eq!(report.reasons[0].code, DiagnosticCode::WebLoginOrChallenge);
    assert!(report.suggested_fallback.is_none());
}
