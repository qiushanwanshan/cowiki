use std::io::Read;
use std::path::Path;

use super::output::AdapterOutput;
use super::xml::xml_visible_text;
use crate::extract::contract::{ExtractStats, OfficeStats};

/// PPTX keeps each slide as its own `ppt/slides/slideN.xml` part. Walk the
/// archive for that pattern (there's no index of slide count elsewhere in
/// the package) and read them back in slide order.
pub fn extract_slides(path: &Path) -> Result<AdapterOutput, String> {
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|error| format!("cannot open PPTX archive: {error}"))?;

    let mut slide_numbers = Vec::new();
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|error| format!("cannot read PPTX archive entry: {error}"))?;
        if let Some(number) = slide_number(entry.name()) {
            slide_numbers.push(number);
        }
    }
    slide_numbers.sort_unstable();

    let slide_count = slide_numbers.len() as u32;
    let mut slides = Vec::new();
    let mut slides_with_text = 0_u32;
    for number in slide_numbers {
        let name = format!("ppt/slides/slide{number}.xml");
        let mut entry = archive
            .by_name(&name)
            .map_err(|error| format!("cannot read {name}: {error}"))?;
        let mut xml = String::new();
        entry
            .read_to_string(&mut xml)
            .map_err(|error| format!("cannot read {name}: {error}"))?;
        let extracted = xml_visible_text(&xml)?;
        if extracted.text.is_empty() {
            continue;
        }
        slides_with_text += 1;
        slides.push(format!("## Slide {number}\n\n{}", extracted.text));
    }

    let office = OfficeStats {
        slide_count,
        slides_with_text,
        heading_count: slides_with_text,
        ..OfficeStats::default()
    };
    let mut stats = ExtractStats {
        heading_count: slides_with_text as usize,
        paragraph_count: slides_with_text as usize,
        office: Some(office),
        ..ExtractStats::default()
    };
    stats.set_coverage(slides_with_text, slide_count);
    Ok(AdapterOutput::with_structure(slides.join("\n\n"), stats))
}

fn slide_number(entry_name: &str) -> Option<u32> {
    entry_name
        .strip_prefix("ppt/slides/slide")?
        .strip_suffix(".xml")?
        .parse()
        .ok()
}
