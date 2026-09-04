//! Office Open XML / ODF helpers shared by slide and ODF adapters.

use std::io::Read;
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::reader::Reader as XmlReader;

use super::output::AdapterOutput;
use crate::extract::contract::{ExtractStats, OfficeStats};

pub struct XmlText {
    pub text: String,
    pub paragraph_count: u32,
    pub heading_count: u32,
    pub non_empty_paragraph_count: u32,
}

pub fn extract_zip_xml_part(path: &Path, part_name: &str) -> Result<AdapterOutput, String> {
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|error| format!("cannot open archive: {error}"))?;
    let mut entry = archive
        .by_name(part_name)
        .map_err(|error| format!("cannot read {part_name}: {error}"))?;
    let mut xml = String::new();
    entry
        .read_to_string(&mut xml)
        .map_err(|error| format!("cannot read {part_name}: {error}"))?;
    let extracted = xml_visible_text(&xml)?;
    let office = OfficeStats {
        paragraph_count: extracted.paragraph_count,
        non_empty_paragraph_count: extracted.non_empty_paragraph_count,
        heading_count: extracted.heading_count,
        ..OfficeStats::default()
    };
    let mut stats = ExtractStats {
        heading_count: extracted.heading_count as usize,
        paragraph_count: extracted.non_empty_paragraph_count as usize,
        office: Some(office),
        ..ExtractStats::default()
    };
    stats.set_coverage(
        extracted.non_empty_paragraph_count,
        extracted.paragraph_count,
    );
    Ok(AdapterOutput::with_structure(extracted.text, stats))
}

/// Walks an XML document's text nodes in order, inserting a line break
/// after every paragraph-like element (`<a:p>` in OOXML, `<text:p>` /
/// `<text:h>` in ODF — matched by local name so both formats share one
/// code path). Good enough for feeding an LLM the reading-order content;
/// it does not attempt to preserve every structural nuance.
pub fn xml_visible_text(xml: &str) -> Result<XmlText, String> {
    let mut reader = XmlReader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut out = String::new();
    let mut buffer = Vec::new();
    let mut paragraph_count = 0_u32;
    let mut heading_count = 0_u32;
    let mut non_empty_paragraph_count = 0_u32;
    let mut current_had_text = false;
    loop {
        match reader
            .read_event_into(&mut buffer)
            .map_err(|error| format!("malformed XML: {error}"))?
        {
            Event::Text(text) => {
                let decoded = text
                    .decode()
                    .map_err(|error| format!("malformed XML text: {error}"))?;
                if push_xml_text(&mut out, &decoded) {
                    current_had_text = true;
                }
            }
            Event::CData(text) => {
                let decoded = text
                    .decode()
                    .map_err(|error| format!("malformed XML text: {error}"))?;
                if push_xml_text(&mut out, &decoded) {
                    current_had_text = true;
                }
            }
            Event::End(end) if matches!(end.local_name().as_ref(), b"p" | b"h") => {
                paragraph_count += 1;
                if end.local_name().as_ref() == b"h" {
                    heading_count += 1;
                }
                if current_had_text {
                    non_empty_paragraph_count += 1;
                }
                current_had_text = false;
                out.push('\n');
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    Ok(XmlText {
        text: out
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
        paragraph_count,
        heading_count,
        non_empty_paragraph_count,
    })
}

fn push_xml_text(out: &mut String, decoded: &str) -> bool {
    if decoded.trim().is_empty() {
        return false;
    }
    if !out.is_empty() && !out.ends_with(['\n', ' ']) {
        out.push(' ');
    }
    out.push_str(decoded.trim());
    true
}
