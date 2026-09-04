//! Serial fallback chain: one adapter at a time, every attempt retained.
//!
//! A later adapter is not assumed to be better. The chain ends by picking
//! the most complete usable candidate, then the one with fewer warnings,
//! then the cheaper extraction mode. Text length is never a tie-breaker.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use super::contract::{ExtractResult, ExtractResultSummary, ExtractionMode, QualityReport};

pub const DEFAULT_MAX_ATTEMPT_DEPTH: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttemptDenied {
    DuplicateAdapter,
    MaxDepth,
}

impl AttemptDenied {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DuplicateAdapter => "duplicate adapter and settings already attempted",
            Self::MaxDepth => "attempt chain reached its maximum depth",
        }
    }
}

impl std::fmt::Display for AttemptDenied {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptRecord {
    pub extractor: String,
    pub extractor_version: String,
    pub settings_hash: String,
    pub mode: ExtractionMode,
    pub duration_ms: u64,
    pub result_summary: ExtractResultSummary,
    pub quality: QualityReport,
    pub result: ExtractResult,
}

impl AttemptRecord {
    pub fn from_result(result: ExtractResult, quality: QualityReport) -> Self {
        Self {
            extractor: result.provenance.extractor.clone(),
            extractor_version: result.provenance.extractor_version.clone(),
            settings_hash: result.provenance.settings_hash.clone(),
            mode: result.provenance.mode,
            duration_ms: result.provenance.duration_ms,
            result_summary: result.summary(),
            quality,
            result,
        }
    }

    fn coverage_bps(&self) -> u32 {
        self.result_summary.coverage_bps.unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainOutcome {
    /// Index into [`attempts`](Self::attempts) of the selected usable
    /// candidate. `None` when every attempt failed.
    pub selected_index: Option<usize>,
    pub attempts: Vec<AttemptRecord>,
}

impl ChainOutcome {
    pub fn selected(&self) -> Option<&AttemptRecord> {
        self.selected_index
            .and_then(|index| self.attempts.get(index))
    }

    pub fn selected_result(&self) -> Option<&ExtractResult> {
        self.selected().map(|attempt| &attempt.result)
    }
}

#[derive(Debug, Clone)]
pub struct AttemptChain {
    attempts: Vec<AttemptRecord>,
    max_depth: usize,
}

impl Default for AttemptChain {
    fn default() -> Self {
        Self::new()
    }
}

impl AttemptChain {
    pub fn new() -> Self {
        Self::with_max_depth(DEFAULT_MAX_ATTEMPT_DEPTH)
    }

    pub fn with_max_depth(max_depth: usize) -> Self {
        Self {
            attempts: Vec::new(),
            max_depth,
        }
    }

    pub fn attempts(&self) -> &[AttemptRecord] {
        &self.attempts
    }

    pub fn can_attempt(&self, extractor: &str, settings_hash: &str) -> Result<(), AttemptDenied> {
        if self
            .attempts
            .iter()
            .any(|attempt| attempt.extractor == extractor && attempt.settings_hash == settings_hash)
        {
            return Err(AttemptDenied::DuplicateAdapter);
        }
        if self.attempts.len() >= self.max_depth {
            return Err(AttemptDenied::MaxDepth);
        }
        Ok(())
    }

    pub fn record(&mut self, record: AttemptRecord) -> Result<(), AttemptDenied> {
        self.can_attempt(&record.extractor, &record.settings_hash)?;
        self.attempts.push(record);
        Ok(())
    }

    /// Pick among retained candidates at the end of the chain. Unusable
    /// outcomes (`fallback` / `fail`) never beat an earlier `pass` or `warn`.
    pub fn select_best(&self) -> Option<usize> {
        self.attempts
            .iter()
            .enumerate()
            .filter(|(_, attempt)| attempt.quality.outcome.is_usable())
            .min_by(|(_, left), (_, right)| {
                right
                    .coverage_bps()
                    .cmp(&left.coverage_bps())
                    .then_with(|| {
                        left.quality
                            .warning_count()
                            .cmp(&right.quality.warning_count())
                    })
                    .then_with(|| left.mode.cost().cmp(&right.mode.cost()))
                    .then_with(|| left.duration_ms.cmp(&right.duration_ms))
            })
            .map(|(index, _)| index)
    }

    pub fn finish(self) -> ChainOutcome {
        let selected_index = self.select_best();
        ChainOutcome {
            selected_index,
            attempts: self.attempts,
        }
    }
}

#[cfg(test)]
mod tests;
