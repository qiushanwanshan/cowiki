use super::*;
use crate::extract::contract::{
    DiagnosticCode, FallbackKind, FallbackTarget, Provenance, QualityClass, QualityMetrics,
    QualityOutcome, QualityReason, SourceFormat,
};

fn record(
    extractor: &str,
    mode: ExtractionMode,
    settings_hash: &str,
    outcome_source: QualityReport,
    coverage_bps: Option<u32>,
    text: &str,
    duration_ms: u64,
) -> AttemptRecord {
    let mut provenance = Provenance::local_fast(extractor, "test", duration_ms);
    provenance.mode = mode;
    provenance.settings_hash = settings_hash.to_string();
    let mut result = ExtractResult::from_text(text, SourceFormat::Pdf, provenance);
    result.stats.coverage_bps = coverage_bps;
    AttemptRecord::from_result(result, outcome_source)
}

fn pass_report(coverage_bps: Option<u32>, warning_count: usize) -> QualityReport {
    QualityReport::pass(QualityMetrics {
        coverage_bps,
        warning_count,
        ..QualityMetrics::default()
    })
}

fn warn_report(coverage_bps: Option<u32>, warning_count: usize) -> QualityReport {
    QualityReport::from_reasons(
        vec![QualityReason::new(
            DiagnosticCode::OcrLowConfidence,
            QualityClass::StructureWarn,
            "low confidence blocks",
        )],
        QualityMetrics {
            coverage_bps,
            warning_count,
            ..QualityMetrics::default()
        },
        None,
    )
}

fn fail_report() -> QualityReport {
    QualityReport::from_reasons(
        vec![QualityReason::new(
            DiagnosticCode::EmptyOutput,
            QualityClass::HardFail,
            "empty",
        )],
        QualityMetrics::default(),
        None,
    )
}

fn fallback_report() -> QualityReport {
    QualityReport::from_reasons(
        vec![QualityReason::new(
            DiagnosticCode::PdfTextlessPage,
            QualityClass::Completeness,
            "no text layer",
        )],
        QualityMetrics::default(),
        Some(FallbackTarget::local(FallbackKind::PageOcr)),
    )
}

#[test]
fn duplicate_adapter_and_settings_are_rejected() {
    let mut chain = AttemptChain::new();
    chain
        .record(record(
            "builtin",
            ExtractionMode::LocalFast,
            "settings-a",
            pass_report(Some(8_000), 0),
            Some(8_000),
            "ok",
            10,
        ))
        .unwrap();

    let denied = chain.can_attempt("builtin", "settings-a").unwrap_err();
    assert_eq!(denied, AttemptDenied::DuplicateAdapter);

    assert!(chain.can_attempt("builtin", "settings-b").is_ok());
    assert!(chain.can_attempt("anydoc", "settings-a").is_ok());
}

#[test]
fn chain_stops_at_configured_depth() {
    let mut chain = AttemptChain::with_max_depth(2);
    chain
        .record(record(
            "builtin",
            ExtractionMode::LocalFast,
            "a",
            fallback_report(),
            Some(1_000),
            "thin",
            10,
        ))
        .unwrap();
    chain
        .record(record(
            "anydoc",
            ExtractionMode::LocalFast,
            "b",
            fail_report(),
            Some(0),
            "",
            20,
        ))
        .unwrap();

    assert_eq!(
        chain.can_attempt("paddleocr", "c").unwrap_err(),
        AttemptDenied::MaxDepth
    );
    assert_eq!(chain.attempts().len(), 2);
}

#[test]
fn every_attempt_is_retained_including_failures() {
    let mut chain = AttemptChain::new();
    chain
        .record(record(
            "builtin",
            ExtractionMode::LocalFast,
            "a",
            fallback_report(),
            Some(2_000),
            "partial",
            10,
        ))
        .unwrap();
    chain
        .record(record(
            "ocr",
            ExtractionMode::LocalOcr,
            "b",
            fail_report(),
            Some(0),
            "",
            40,
        ))
        .unwrap();

    let outcome = chain.finish();
    assert_eq!(outcome.attempts.len(), 2);
    assert_eq!(
        outcome.attempts[0].quality.outcome,
        QualityOutcome::Fallback
    );
    assert_eq!(outcome.attempts[1].quality.outcome, QualityOutcome::Fail);
    assert!(outcome.selected_index.is_none());
}

#[test]
fn later_unusable_result_does_not_replace_an_earlier_warning() {
    let mut chain = AttemptChain::new();
    chain
        .record(record(
            "builtin",
            ExtractionMode::LocalFast,
            "a",
            warn_report(Some(7_000), 1),
            Some(7_000),
            "usable",
            10,
        ))
        .unwrap();
    chain
        .record(record(
            "ocr",
            ExtractionMode::LocalOcr,
            "b",
            fail_report(),
            Some(9_000),
            "unreadable",
            80,
        ))
        .unwrap();

    let outcome = chain.finish();
    assert_eq!(outcome.selected_index, Some(0));
    assert_eq!(outcome.selected().unwrap().extractor, "builtin");
}

#[test]
fn completeness_beats_a_later_heavier_adapter() {
    let mut chain = AttemptChain::new();
    chain
        .record(record(
            "builtin",
            ExtractionMode::LocalFast,
            "a",
            pass_report(Some(9_500), 0),
            Some(9_500),
            "short but complete",
            8,
        ))
        .unwrap();
    chain
        .record(record(
            "ocr",
            ExtractionMode::LocalOcr,
            "b",
            pass_report(Some(6_000), 0),
            Some(6_000),
            "much longer text that still missed pages",
            120,
        ))
        .unwrap();

    let outcome = chain.finish();
    assert_eq!(outcome.selected_index, Some(0));
}

#[test]
fn text_length_is_not_a_tie_breaker() {
    let mut chain = AttemptChain::new();
    chain
        .record(record(
            "builtin",
            ExtractionMode::LocalFast,
            "a",
            pass_report(Some(8_000), 0),
            Some(8_000),
            "short",
            10,
        ))
        .unwrap();
    chain
        .record(record(
            "ocr",
            ExtractionMode::LocalOcr,
            "b",
            pass_report(Some(8_000), 0),
            Some(8_000),
            "a much longer transcription of the same pages",
            90,
        ))
        .unwrap();

    let outcome = chain.finish();
    assert_eq!(outcome.selected().unwrap().extractor, "builtin");
    assert!(outcome.selected().unwrap().result.text.len() < outcome.attempts[1].result.text.len());
}

#[test]
fn higher_coverage_does_win_even_when_it_costs_more() {
    let mut chain = AttemptChain::new();
    chain
        .record(record(
            "builtin",
            ExtractionMode::LocalFast,
            "a",
            pass_report(Some(4_000), 0),
            Some(4_000),
            "missing sheets",
            10,
        ))
        .unwrap();
    chain
        .record(record(
            "anydoc",
            ExtractionMode::LocalFast,
            "b",
            pass_report(Some(9_000), 0),
            Some(9_000),
            "recovered sheets",
            40,
        ))
        .unwrap();

    assert_eq!(chain.finish().selected().unwrap().extractor, "anydoc");
}
