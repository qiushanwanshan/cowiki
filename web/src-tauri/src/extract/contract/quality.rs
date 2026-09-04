//! Quality-gate verdicts. A later adapter is chosen by these outcomes,
//! never by “the output looks longer”.

use serde::{Deserialize, Serialize};

use super::{DiagnosticCode, ExtractStats, SourceLocation};

/// How badly a quality reason should affect the gate. Higher values win
/// when several reasons are present: a hard fail is never hidden by a
/// coverage warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum QualityClass {
    StructureWarn = 0,
    Readability = 1,
    Completeness = 2,
    HardFail = 3,
}

/// Gate verdict. `fallback` means "try the suggested adapter"; it is not a
/// successful Source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QualityOutcome {
    Pass,
    Warn,
    Fallback,
    Fail,
}

impl QualityOutcome {
    pub fn is_usable(self) -> bool {
        matches!(self, Self::Pass | Self::Warn)
    }
}

/// Adapter capability to try next. Cloud targets must stay opt-in per
/// document; the scheduler is responsible for checking consent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FallbackKind {
    AnyDoc,
    PageOcr,
    LayoutParser,
    SemanticBody,
    IsolatedBrowser,
    Cloud,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FallbackTarget {
    pub kind: FallbackKind,
    /// Capability pack / sidecar id, when the target is not built in.
    pub requires_capability: Option<String>,
    pub requires_consent: bool,
}

impl FallbackTarget {
    pub fn local(kind: FallbackKind) -> Self {
        Self {
            kind,
            requires_capability: None,
            requires_consent: false,
        }
    }

    pub fn capability(kind: FallbackKind, capability: impl Into<String>) -> Self {
        Self {
            kind,
            requires_capability: Some(capability.into()),
            requires_consent: false,
        }
    }

    pub fn cloud(kind: FallbackKind) -> Self {
        Self {
            kind,
            requires_capability: None,
            requires_consent: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityReason {
    pub code: DiagnosticCode,
    pub class: QualityClass,
    pub message: String,
    pub location: Option<SourceLocation>,
}

impl QualityReason {
    pub fn new(code: DiagnosticCode, class: QualityClass, message: impl Into<String>) -> Self {
        Self {
            code,
            class,
            message: message.into(),
            location: None,
        }
    }

    pub fn at(mut self, location: SourceLocation) -> Self {
        self.location = Some(location);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QualityMetrics {
    pub replacement_char_ratio: f32,
    pub control_char_ratio: f32,
    pub coverage_bps: Option<u32>,
    pub warning_count: usize,
}

impl QualityMetrics {
    pub fn from_stats(stats: &ExtractStats, warning_count: usize) -> Self {
        Self {
            replacement_char_ratio: stats.replacement_char_ratio(),
            control_char_ratio: stats.control_char_ratio(),
            coverage_bps: stats.coverage_bps,
            warning_count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityReport {
    pub outcome: QualityOutcome,
    pub reasons: Vec<QualityReason>,
    pub metrics: QualityMetrics,
    pub suggested_fallback: Option<FallbackTarget>,
}

impl QualityReport {
    pub fn from_reasons(
        reasons: Vec<QualityReason>,
        metrics: QualityMetrics,
        suggested_fallback: Option<FallbackTarget>,
    ) -> Self {
        let outcome = resolve_quality_outcome(&reasons, suggested_fallback.as_ref());
        Self {
            outcome,
            reasons,
            metrics,
            suggested_fallback,
        }
    }

    pub fn pass(metrics: QualityMetrics) -> Self {
        Self {
            outcome: QualityOutcome::Pass,
            reasons: Vec::new(),
            metrics,
            suggested_fallback: None,
        }
    }

    pub fn warning_count(&self) -> usize {
        self.reasons
            .iter()
            .filter(|reason| reason.class == QualityClass::StructureWarn)
            .count()
    }
}

/// Decide the gate outcome from the highest-priority reason class.
/// Completeness and readability become `fallback` only when a next adapter
/// is actually available; otherwise they fail rather than look successful.
pub fn resolve_quality_outcome(
    reasons: &[QualityReason],
    suggested_fallback: Option<&FallbackTarget>,
) -> QualityOutcome {
    let Some(highest) = reasons.iter().map(|reason| reason.class).max() else {
        return QualityOutcome::Pass;
    };
    match highest {
        QualityClass::HardFail => QualityOutcome::Fail,
        QualityClass::Completeness | QualityClass::Readability => {
            if suggested_fallback.is_some() {
                QualityOutcome::Fallback
            } else {
                QualityOutcome::Fail
            }
        }
        QualityClass::StructureWarn => QualityOutcome::Warn,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hard_fail_outranks_every_other_quality_class() {
        let reasons = vec![
            QualityReason::new(
                DiagnosticCode::DocxParagraphCoverageLow,
                QualityClass::Completeness,
                "coverage low",
            ),
            QualityReason::new(
                DiagnosticCode::ExtractorTimeout,
                QualityClass::HardFail,
                "timed out",
            ),
            QualityReason::new(
                DiagnosticCode::PdfReadingOrderSuspect,
                QualityClass::StructureWarn,
                "order suspect",
            ),
        ];
        assert_eq!(
            resolve_quality_outcome(&reasons, Some(&FallbackTarget::local(FallbackKind::AnyDoc))),
            QualityOutcome::Fail
        );
    }

    #[test]
    fn completeness_becomes_fallback_only_when_a_target_exists() {
        let reasons = vec![QualityReason::new(
            DiagnosticCode::XlsxSheetMissing,
            QualityClass::Completeness,
            "sheet missing",
        )];
        assert_eq!(
            resolve_quality_outcome(&reasons, Some(&FallbackTarget::local(FallbackKind::AnyDoc))),
            QualityOutcome::Fallback
        );
        assert_eq!(
            resolve_quality_outcome(&reasons, None),
            QualityOutcome::Fail
        );
    }

    #[test]
    fn readability_outranks_structure_warnings() {
        let reasons = vec![
            QualityReason::new(
                DiagnosticCode::DocxTableMissing,
                QualityClass::StructureWarn,
                "table degraded",
            ),
            QualityReason::new(
                DiagnosticCode::ReplacementCharRatioHigh,
                QualityClass::Readability,
                "garbled",
            ),
        ];
        assert_eq!(
            resolve_quality_outcome(
                &reasons,
                Some(&FallbackTarget::local(FallbackKind::PageOcr))
            ),
            QualityOutcome::Fallback
        );
    }

    #[test]
    fn structure_warnings_alone_are_warn_not_fail() {
        let reasons = vec![QualityReason::new(
            DiagnosticCode::OcrLowConfidence,
            QualityClass::StructureWarn,
            "a few blocks are uncertain",
        )];
        assert_eq!(
            resolve_quality_outcome(
                &reasons,
                Some(&FallbackTarget::local(FallbackKind::PageOcr))
            ),
            QualityOutcome::Warn
        );
    }

    #[test]
    fn empty_reasons_pass() {
        assert_eq!(resolve_quality_outcome(&[], None), QualityOutcome::Pass);
    }
}
