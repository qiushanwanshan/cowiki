//! Unified extract contract: structured results, diagnostics, and quality reports.
//!
//! Adapters return these types; they must not write Source files themselves.
//! Quality gates consume stats and diagnostics — they do not need a full
//! document AST — so [`ExtractResult::blocks`] may stay empty while current
//! adapters still fill [`ExtractResult::text`].

#![allow(dead_code)]

mod diagnostic;
mod quality;
mod result;
mod stats;

pub use diagnostic::{Diagnostic, DiagnosticCode};
pub use quality::{
    resolve_quality_outcome, FallbackKind, FallbackTarget, QualityClass, QualityMetrics,
    QualityOutcome, QualityReason, QualityReport,
};
pub use result::{
    AssetKind, AssetRef, Block, BlockContent, BlockKind, DocumentMeta, ExtractResult,
    ExtractResultSummary, ExtractionMode, Provenance, SourceFormat, SourceLocation,
    MARKDOWN_SPEC_VERSION, QUALITY_POLICY_VERSION,
};
pub use stats::{ExtractStats, OfficeStats, PdfPageStats, PdfStats, SpreadsheetStats, WebStats};
