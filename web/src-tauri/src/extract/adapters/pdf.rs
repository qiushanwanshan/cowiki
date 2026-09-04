use std::path::Path;

use super::output::AdapterOutput;
use crate::extract::contract::{ExtractStats, PdfPageStats, PdfStats};

const MIN_VALID_CHARS_PER_PAGE: u32 = 20;

pub fn extract_pdf(path: &Path) -> Result<AdapterOutput, String> {
    let text = pdf_extract::extract_text(path)
        .map_err(|error| format!("cannot extract text from PDF: {error}"))?;
    let pages = pdf_extract::extract_text_by_pages(path).unwrap_or_default();
    let mut stats = ExtractStats::default();
    if !pages.is_empty() {
        let pdf = pdf_stats_from_pages(&pages);
        stats.set_coverage(pdf.pages_with_valid_text, pdf.page_count);
        stats.pdf = Some(pdf);
    }
    Ok(AdapterOutput::with_structure(text, stats))
}

fn pdf_stats_from_pages(pages: &[String]) -> PdfStats {
    let page_stats: Vec<PdfPageStats> = pages
        .iter()
        .enumerate()
        .map(|(index, page)| {
            let total = page.chars().count() as u32;
            let replacement = page.chars().filter(|&ch| ch == '\u{FFFD}').count() as u32;
            let valid_chars = ExtractStats::from_text(page).valid_char_count as u32;
            PdfPageStats {
                page: (index + 1) as u32,
                valid_chars,
                invalid_glyph_ratio: if total == 0 {
                    0.0
                } else {
                    replacement as f32 / total as f32
                },
                image_coverage: 0.0,
            }
        })
        .collect();
    let pages_with_valid_text = page_stats
        .iter()
        .filter(|page| page.valid_chars >= MIN_VALID_CHARS_PER_PAGE)
        .count() as u32;
    PdfStats {
        page_count: page_stats.len() as u32,
        pages_with_valid_text,
        pages: page_stats,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_stats_count_valid_text_and_replacement_glyphs() {
        let stats = pdf_stats_from_pages(&[
            "a short page".to_string(),
            "This page has enough readable characters to count as valid.".to_string(),
            "\u{FFFD}\u{FFFD} garbled".to_string(),
        ]);
        assert_eq!(stats.page_count, 3);
        assert_eq!(stats.pages_with_valid_text, 1);
        assert_eq!(stats.pages[0].valid_chars, 12);
        assert!(stats.pages[2].invalid_glyph_ratio > 0.0);
    }
}
