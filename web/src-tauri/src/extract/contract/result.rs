use serde::{Deserialize, Serialize};

use super::{Diagnostic, DiagnosticCode, ExtractStats};

/// Policy version recorded in provenance so old Source files can be explained
/// after quality thresholds change.
pub const QUALITY_POLICY_VERSION: &str = "1";

/// Markdown renderer/spec version recorded in provenance so a later spec
/// change can trigger a targeted re-render instead of a silent format drift.
pub const MARKDOWN_SPEC_VERSION: &str = "1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceFormat {
    Pdf,
    Docx,
    Xlsx,
    Xls,
    Ods,
    Pptx,
    Odt,
    Odp,
    Text,
    Html,
    Url,
    Image,
    Unknown,
}

impl Default for SourceFormat {
    fn default() -> Self {
        Self::Unknown
    }
}

impl SourceFormat {
    pub fn from_extension(extension: &str) -> Self {
        match extension {
            "pdf" => Self::Pdf,
            "docx" => Self::Docx,
            "xlsx" => Self::Xlsx,
            "xls" => Self::Xls,
            "ods" => Self::Ods,
            "pptx" => Self::Pptx,
            "odt" => Self::Odt,
            "odp" => Self::Odp,
            "html" | "htm" => Self::Html,
            "md" | "mdx" | "txt" | "csv" | "tsv" | "json" | "xml" | "yaml" | "yml" => Self::Text,
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "tif" | "tiff" | "bmp" => Self::Image,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExtractionMode {
    LocalFast,
    LocalOcr,
    LocalBrowser,
    Cloud,
}

impl ExtractionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalFast => "local-fast",
            Self::LocalOcr => "local-ocr",
            Self::LocalBrowser => "local-browser",
            Self::Cloud => "cloud",
        }
    }

    /// Relative resource cost used only to break ties when completeness
    /// and warning counts are equal. Lower is cheaper.
    pub fn cost(self) -> u8 {
        match self {
            Self::LocalFast => 0,
            Self::LocalOcr => 1,
            Self::LocalBrowser => 2,
            Self::Cloud => 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provenance {
    pub extractor: String,
    pub extractor_version: String,
    pub mode: ExtractionMode,
    pub settings_hash: String,
    pub duration_ms: u64,
    pub quality_policy_version: String,
    pub markdown_spec_version: String,
}

impl Provenance {
    pub fn local_fast(
        extractor: impl Into<String>,
        extractor_version: impl Into<String>,
        duration_ms: u64,
    ) -> Self {
        Self {
            extractor: extractor.into(),
            extractor_version: extractor_version.into(),
            mode: ExtractionMode::LocalFast,
            settings_hash: String::new(),
            duration_ms,
            quality_policy_version: QUALITY_POLICY_VERSION.to_string(),
            markdown_spec_version: MARKDOWN_SPEC_VERSION.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DocumentMeta {
    pub format: SourceFormat,
    pub title: Option<String>,
    pub page_count: Option<u32>,
    pub language_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SourceLocation {
    pub page: Option<u32>,
    pub slide: Option<u32>,
    pub sheet: Option<String>,
    pub cell: Option<String>,
    pub bbox: Option<[f32; 4]>,
}

impl SourceLocation {
    pub fn page(page: u32) -> Self {
        Self {
            page: Some(page),
            ..Self::default()
        }
    }

    pub fn slide(slide: u32) -> Self {
        Self {
            slide: Some(slide),
            ..Self::default()
        }
    }

    pub fn sheet(sheet: impl Into<String>) -> Self {
        Self {
            sheet: Some(sheet.into()),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BlockKind {
    Heading,
    Paragraph,
    List,
    Table,
    Code,
    Image,
    Equation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum BlockContent {
    Text {
        text: String,
    },
    Heading {
        level: u8,
        text: String,
    },
    List {
        ordered: bool,
        items: Vec<String>,
    },
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    Code {
        language: Option<String>,
        text: String,
    },
    Image {
        alt: String,
        href: String,
        caption: Option<String>,
    },
    Equation {
        latex: String,
        display: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    pub kind: BlockKind,
    pub content: BlockContent,
    pub location: Option<SourceLocation>,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AssetKind {
    Image,
    Attachment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetRef {
    pub id: String,
    pub kind: AssetKind,
    pub href: String,
    pub alt: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractResult {
    pub document: DocumentMeta,
    pub blocks: Vec<Block>,
    pub assets: Vec<AssetRef>,
    pub diagnostics: Vec<Diagnostic>,
    pub stats: ExtractStats,
    pub provenance: Provenance,
    /// Current adapters emit a single text blob. Prefer this when `blocks`
    /// is empty; the unified renderer will replace it later.
    pub text: String,
}

impl ExtractResult {
    pub fn from_text(
        text: impl Into<String>,
        format: SourceFormat,
        provenance: Provenance,
    ) -> Self {
        Self::from_adapter(text, format, ExtractStats::default(), provenance)
    }

    pub fn from_adapter(
        text: impl Into<String>,
        format: SourceFormat,
        mut stats: ExtractStats,
        provenance: Provenance,
    ) -> Self {
        let text = text.into();
        stats.apply_text_counts(&text);
        let page_count = stats.pdf.as_ref().map(|pdf| pdf.page_count);
        Self {
            document: DocumentMeta {
                format,
                page_count,
                ..DocumentMeta::default()
            },
            blocks: Vec::new(),
            assets: Vec::new(),
            diagnostics: Vec::new(),
            stats,
            provenance,
            text,
        }
    }

    pub fn output_text(&self) -> &str {
        &self.text
    }

    pub fn summary(&self) -> ExtractResultSummary {
        ExtractResultSummary {
            char_count: self.stats.char_count,
            block_count: self.blocks.len(),
            coverage_bps: self.stats.coverage_bps,
            diagnostic_codes: self.diagnostics.iter().map(|item| item.code).collect(),
        }
    }
}

/// Log-safe subset of an extract attempt. Does not include body text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractResultSummary {
    pub char_count: usize,
    pub block_count: usize,
    pub coverage_bps: Option<u32>,
    pub diagnostic_codes: Vec<DiagnosticCode>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_text_fills_stats_and_leaves_blocks_empty() {
        let result = ExtractResult::from_text(
            "Hello\u{FFFD} world",
            SourceFormat::Text,
            Provenance::local_fast("builtin", "test", 1),
        );
        assert!(result.blocks.is_empty());
        assert_eq!(result.stats.replacement_char_count, 1);
        assert_eq!(result.output_text(), "Hello\u{FFFD} world");
        assert_eq!(result.summary().char_count, result.stats.char_count);
        assert!(result.summary().diagnostic_codes.is_empty());
    }

    #[test]
    fn quality_report_records_policy_versions_on_provenance() {
        let provenance = Provenance::local_fast("builtin", "0.1.0", 12);
        assert_eq!(provenance.quality_policy_version, QUALITY_POLICY_VERSION);
        assert_eq!(provenance.markdown_spec_version, MARKDOWN_SPEC_VERSION);
        assert_eq!(provenance.mode, ExtractionMode::LocalFast);
        assert_eq!(provenance.mode.as_str(), "local-fast");
    }
}
