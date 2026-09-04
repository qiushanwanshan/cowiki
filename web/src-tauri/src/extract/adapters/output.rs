use crate::extract::contract::ExtractStats;

/// Text plus the structure counts an adapter can see without changing
/// the string it already produced.
#[derive(Debug, Clone)]
pub struct AdapterOutput {
    pub text: String,
    pub stats: ExtractStats,
}

impl AdapterOutput {
    pub fn from_text(text: impl Into<String>) -> Self {
        let text = text.into();
        let stats = ExtractStats::from_text(&text);
        Self { text, stats }
    }

    pub fn with_structure(text: impl Into<String>, mut stats: ExtractStats) -> Self {
        let text = text.into();
        stats.apply_text_counts(&text);
        Self { text, stats }
    }
}
