mod anydoc;
mod docx;
mod native_ocr;
mod output;
mod pdf;
mod slides;
mod spreadsheet;
mod web;
mod xml;

pub use anydoc::{extract_anydoc, is_anydoc_primary, ANYDOC_EXTRACTOR, ANYDOC_SETTINGS_HASH};
pub use docx::extract_docx;
pub use native_ocr::extract_image_result as extract_ocr_image_result;
pub use output::AdapterOutput;
pub use pdf::extract_pdf;
pub use slides::extract_slides;
pub use spreadsheet::extract_spreadsheet;
pub use web::{extract_html_file, extract_web_page};
pub use xml::extract_zip_xml_part;
