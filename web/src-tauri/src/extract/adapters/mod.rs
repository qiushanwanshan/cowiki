mod docx;
mod output;
mod pdf;
mod slides;
mod spreadsheet;
mod web;
mod xml;

pub use docx::extract_docx;
pub use output::AdapterOutput;
pub use pdf::extract_pdf;
pub use slides::extract_slides;
pub use spreadsheet::extract_spreadsheet;
pub use web::{extract_html_file, extract_web_page};
pub use xml::extract_zip_xml_part;
