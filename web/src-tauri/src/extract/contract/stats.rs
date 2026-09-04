use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PdfPageStats {
    pub page: u32,
    pub valid_chars: u32,
    pub invalid_glyph_ratio: f32,
    pub image_coverage: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PdfStats {
    pub page_count: u32,
    pub pages_with_valid_text: u32,
    pub pages: Vec<PdfPageStats>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OfficeStats {
    pub paragraph_count: u32,
    pub non_empty_paragraph_count: u32,
    pub heading_count: u32,
    pub table_count: u32,
    pub table_row_count: u32,
    pub table_cell_count: u32,
    pub image_count: u32,
    pub slide_count: u32,
    pub slides_with_text: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SpreadsheetStats {
    pub sheet_count: u32,
    pub visible_sheet_count: u32,
    pub non_empty_cell_count: u32,
    pub formula_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WebStats {
    pub http_status: Option<u16>,
    pub final_url: Option<String>,
    pub response_bytes: u64,
    pub article_text_chars: u32,
    pub article_to_body_text_ratio: f32,
    pub link_text_ratio: f32,
    pub readability_succeeded: bool,
    pub dom_visible_text_chars: u32,
    pub spa_shell_detected: bool,
    pub login_or_challenge_detected: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExtractStats {
    pub char_count: usize,
    pub valid_char_count: usize,
    pub replacement_char_count: usize,
    pub control_char_count: usize,
    pub heading_count: usize,
    pub paragraph_count: usize,
    pub table_count: usize,
    pub image_count: usize,
    pub list_count: usize,
    /// 0–10_000 basis points. Completeness of source structure covered by
    /// the output. Missing means the adapter did not compute coverage yet.
    pub coverage_bps: Option<u32>,
    pub pdf: Option<PdfStats>,
    pub office: Option<OfficeStats>,
    pub spreadsheet: Option<SpreadsheetStats>,
    pub web: Option<WebStats>,
}

impl ExtractStats {
    pub fn from_text(text: &str) -> Self {
        let mut stats = Self::default();
        stats.apply_text_counts(text);
        stats
    }

    pub fn apply_text_counts(&mut self, text: &str) {
        self.char_count = text.chars().count();
        self.replacement_char_count = text.chars().filter(|&ch| ch == '\u{FFFD}').count();
        self.control_char_count = text
            .chars()
            .filter(|ch| ch.is_control() && *ch != '\n' && *ch != '\t' && *ch != '\r')
            .count();
        self.valid_char_count = self
            .char_count
            .saturating_sub(self.replacement_char_count + self.control_char_count);
    }

    pub fn set_coverage(&mut self, covered: u32, total: u32) {
        self.coverage_bps = if total == 0 {
            None
        } else {
            Some(((u64::from(covered) * 10_000) / u64::from(total)) as u32)
        };
    }

    pub fn replacement_char_ratio(&self) -> f32 {
        ratio(self.replacement_char_count, self.char_count)
    }

    pub fn control_char_ratio(&self) -> f32 {
        ratio(self.control_char_count, self.char_count)
    }
}

fn ratio(part: usize, whole: usize) -> f32 {
    if whole == 0 {
        0.0
    } else {
        part as f32 / whole as f32
    }
}
