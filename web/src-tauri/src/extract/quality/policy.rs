//! First-round engineering defaults for quality gates.
//!
//! These thresholds exist so the rules can ship and be tested. Calibrate
//! them against CoWiki corpus before treating any value as a product standard.

/// Replacement characters above this share of output trigger readability fallback.
pub const REPLACEMENT_CHAR_RATIO_LIMIT: f32 = 0.01;
/// Illegal control characters above this share of output trigger readability fallback.
pub const CONTROL_CHAR_RATIO_LIMIT: f32 = 0.01;
/// A PDF page below this many valid characters is treated as textless.
pub const PDF_MIN_VALID_CHARS_PER_PAGE: u32 = 20;
/// Textless pages at or above this share of the document are mixed/scanned.
pub const PDF_TEXTLESS_PAGE_RATIO_LIMIT: f32 = 0.30;
/// Per-page invalid glyph share that is worth a page-level warning.
pub const PDF_INVALID_GLYPH_RATIO_LIMIT: f32 = 0.01;
/// Structure coverage below this (basis points) is a completeness failure.
pub const COVERAGE_FALLBACK_BPS: u32 = 7_000;
/// Full coverage in basis points (100.00%).
pub const COVERAGE_FULL_BPS: u32 = 10_000;
/// Output shorter than this, against a large source, is suspiciously truncated.
pub const OUTPUT_TOO_SHORT_CHARS: usize = 40;
/// Consecutive identical blocks at or above this run count as repeated content.
pub const REPEATED_BLOCK_RUN: usize = 3;
/// Only lines at least this long participate in the repeated-block check.
pub const REPEATED_BLOCK_MIN_CHARS: usize = 20;
pub const SUBSTANTIAL_PDF_PAGES: u32 = 2;
pub const SUBSTANTIAL_PARAGRAPHS: u32 = 8;
pub const SUBSTANTIAL_SLIDES: u32 = 3;
pub const SUBSTANTIAL_SHEETS: u32 = 2;
pub const SUBSTANTIAL_CELLS: u32 = 20;
/// HTML larger than this with almost no article text is treated as a failed extract.
pub const WEB_LARGE_HTML_BYTES: u64 = 10 * 1024;
/// Article shorter than this against a large page triggers the semantic-body fallback.
pub const WEB_MIN_ARTICLE_CHARS: u32 = 200;
/// Link text above this share of visible text is treated as navigation/index chrome.
pub const WEB_LINK_TEXT_RATIO_LIMIT: f32 = 0.60;
/// Readability output below this share of visible body text is not the article.
pub const WEB_ARTICLE_TO_BODY_RATIO_LIMIT: f32 = 0.15;
