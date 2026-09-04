//! Stable diagnostic codes. Serialized as the SCREAMING_SNAKE identifiers
//! used by tests, telemetry, and UI copy. Add new variants; do not rename
//! existing ones.

use serde::{Deserialize, Serialize};

use super::SourceLocation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiagnosticCode {
    EmptyOutput,
    BudgetExceeded,
    EncryptedInput,
    TypeMismatch,
    ParserCrash,
    MarkdownRenderFailed,
    ExtractorTimeout,
    ExtractorCancelled,
    CapabilityUnavailable,
    ReplacementCharRatioHigh,
    ControlCharRatioHigh,
    OutputTooShort,
    RepeatedContent,
    TemplateOutput,
    PdfTextlessPage,
    PdfInvalidGlyphRatioHigh,
    PdfReadingOrderSuspect,
    PdfScannedDocument,
    PdfMixedDocument,
    DocxParagraphCoverageLow,
    DocxTableMissing,
    PptxSlideMissing,
    XlsxSheetMissing,
    XlsxCellCoverageLow,
    WebReadabilityEmpty,
    WebSpaShellDetected,
    WebNavigationDensityHigh,
    WebLoginOrChallenge,
    OcrLanguageUnavailable,
    OcrLowConfidence,
}

impl DiagnosticCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EmptyOutput => "EMPTY_OUTPUT",
            Self::BudgetExceeded => "BUDGET_EXCEEDED",
            Self::EncryptedInput => "ENCRYPTED_INPUT",
            Self::TypeMismatch => "TYPE_MISMATCH",
            Self::ParserCrash => "PARSER_CRASH",
            Self::MarkdownRenderFailed => "MARKDOWN_RENDER_FAILED",
            Self::ExtractorTimeout => "EXTRACTOR_TIMEOUT",
            Self::ExtractorCancelled => "EXTRACTOR_CANCELLED",
            Self::CapabilityUnavailable => "CAPABILITY_UNAVAILABLE",
            Self::ReplacementCharRatioHigh => "REPLACEMENT_CHAR_RATIO_HIGH",
            Self::ControlCharRatioHigh => "CONTROL_CHAR_RATIO_HIGH",
            Self::OutputTooShort => "OUTPUT_TOO_SHORT",
            Self::RepeatedContent => "REPEATED_CONTENT",
            Self::TemplateOutput => "TEMPLATE_OUTPUT",
            Self::PdfTextlessPage => "PDF_TEXTLESS_PAGE",
            Self::PdfInvalidGlyphRatioHigh => "PDF_INVALID_GLYPH_RATIO_HIGH",
            Self::PdfReadingOrderSuspect => "PDF_READING_ORDER_SUSPECT",
            Self::PdfScannedDocument => "PDF_SCANNED_DOCUMENT",
            Self::PdfMixedDocument => "PDF_MIXED_DOCUMENT",
            Self::DocxParagraphCoverageLow => "DOCX_PARAGRAPH_COVERAGE_LOW",
            Self::DocxTableMissing => "DOCX_TABLE_MISSING",
            Self::PptxSlideMissing => "PPTX_SLIDE_MISSING",
            Self::XlsxSheetMissing => "XLSX_SHEET_MISSING",
            Self::XlsxCellCoverageLow => "XLSX_CELL_COVERAGE_LOW",
            Self::WebReadabilityEmpty => "WEB_READABILITY_EMPTY",
            Self::WebSpaShellDetected => "WEB_SPA_SHELL_DETECTED",
            Self::WebNavigationDensityHigh => "WEB_NAVIGATION_DENSITY_HIGH",
            Self::WebLoginOrChallenge => "WEB_LOGIN_OR_CHALLENGE",
            Self::OcrLanguageUnavailable => "OCR_LANGUAGE_UNAVAILABLE",
            Self::OcrLowConfidence => "OCR_LOW_CONFIDENCE",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub message: String,
    pub location: Option<SourceLocation>,
}

impl Diagnostic {
    pub fn new(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            location: None,
        }
    }

    pub fn at(mut self, location: SourceLocation) -> Self {
        self.location = Some(location);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_codes_serialize_to_stable_identifiers() {
        let cases = [
            (DiagnosticCode::PdfTextlessPage, "PDF_TEXTLESS_PAGE"),
            (
                DiagnosticCode::PdfInvalidGlyphRatioHigh,
                "PDF_INVALID_GLYPH_RATIO_HIGH",
            ),
            (
                DiagnosticCode::PdfReadingOrderSuspect,
                "PDF_READING_ORDER_SUSPECT",
            ),
            (
                DiagnosticCode::DocxParagraphCoverageLow,
                "DOCX_PARAGRAPH_COVERAGE_LOW",
            ),
            (DiagnosticCode::XlsxSheetMissing, "XLSX_SHEET_MISSING"),
            (DiagnosticCode::WebReadabilityEmpty, "WEB_READABILITY_EMPTY"),
            (
                DiagnosticCode::WebSpaShellDetected,
                "WEB_SPA_SHELL_DETECTED",
            ),
            (
                DiagnosticCode::WebNavigationDensityHigh,
                "WEB_NAVIGATION_DENSITY_HIGH",
            ),
            (
                DiagnosticCode::OcrLanguageUnavailable,
                "OCR_LANGUAGE_UNAVAILABLE",
            ),
            (DiagnosticCode::ExtractorTimeout, "EXTRACTOR_TIMEOUT"),
        ];
        for (code, expected) in cases {
            assert_eq!(code.as_str(), expected);
            assert_eq!(
                serde_json::to_string(&code).unwrap(),
                format!("\"{expected}\"")
            );
            assert_eq!(
                serde_json::from_str::<DiagnosticCode>(&format!("\"{expected}\"")).unwrap(),
                code
            );
        }
    }
}
