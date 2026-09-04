use std::path::Path;

use super::output::AdapterOutput;
use crate::extract::contract::{ExtractStats, OfficeStats};

pub fn extract_docx(path: &Path) -> Result<AdapterOutput, String> {
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    let docx =
        docx_rs::read_docx(&bytes).map_err(|error| format!("cannot parse DOCX: {error:?}"))?;

    let mut lines = Vec::new();
    let mut office = OfficeStats::default();
    let mut list_count = 0_usize;
    for child in docx.document.children {
        match child {
            docx_rs::DocumentChild::Paragraph(paragraph) => {
                office.paragraph_count += 1;
                match docx_paragraph_text(&paragraph) {
                    Some(line) => {
                        office.non_empty_paragraph_count += 1;
                        if line.starts_with('#') {
                            office.heading_count += 1;
                        }
                        if line.starts_with("- ") {
                            list_count += 1;
                        }
                        lines.push(line);
                    }
                    None => {}
                }
            }
            docx_rs::DocumentChild::Table(table) => {
                office.table_count += 1;
                office.table_row_count += table.rows.len() as u32;
                office.table_cell_count += table
                    .rows
                    .iter()
                    .map(|row| {
                        let docx_rs::TableChild::TableRow(row) = row;
                        row.cells.len() as u32
                    })
                    .sum::<u32>();
                lines.push(docx_table_text(&table));
            }
            _ => {}
        }
    }

    let mut stats = ExtractStats {
        heading_count: office.heading_count as usize,
        paragraph_count: office.non_empty_paragraph_count as usize,
        table_count: office.table_count as usize,
        list_count,
        office: Some(office.clone()),
        ..ExtractStats::default()
    };
    stats.set_coverage(office.non_empty_paragraph_count, office.paragraph_count);
    Ok(AdapterOutput::with_structure(lines.join("\n\n"), stats))
}

/// A paragraph styled "HeadingN" becomes a Markdown heading; everything
/// else is a plain line. Run-level bold/italic markers are deliberately
/// skipped — they add fragility (a run boundary mid-word turns into
/// mangled `**` pairs) for little value in text meant for an LLM to read
/// and reorganize, not for byte-faithful document reproduction.
fn docx_paragraph_text(paragraph: &docx_rs::Paragraph) -> Option<String> {
    let text: String = paragraph
        .children
        .iter()
        .filter_map(|child| match child {
            docx_rs::ParagraphChild::Run(run) => Some(docx_run_text(run)),
            _ => None,
        })
        .collect();
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    let heading_level = paragraph
        .property
        .style
        .as_ref()
        .and_then(|style| style.val.strip_prefix("Heading"))
        .and_then(|level| level.trim().parse::<usize>().ok())
        .map(|level| level.clamp(1, 6));

    Some(match heading_level {
        Some(level) => format!("{} {text}", "#".repeat(level)),
        None if paragraph.property.numbering_property.is_some() => format!("- {text}"),
        None => text.to_string(),
    })
}

fn docx_run_text(run: &docx_rs::Run) -> String {
    run.children
        .iter()
        .filter_map(|child| match child {
            docx_rs::RunChild::Text(text) => Some(text.text.as_str()),
            docx_rs::RunChild::Tab(_) => Some("\t"),
            _ => None,
        })
        .collect()
}

fn docx_table_text(table: &docx_rs::Table) -> String {
    table
        .rows
        .iter()
        .map(|row| {
            let docx_rs::TableChild::TableRow(row) = row;
            row.cells
                .iter()
                .map(|cell| {
                    let docx_rs::TableRowChild::TableCell(cell) = cell;
                    cell.children
                        .iter()
                        .filter_map(|content| match content {
                            docx_rs::TableCellContent::Paragraph(paragraph) => {
                                docx_paragraph_text(paragraph)
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect::<Vec<_>>()
                .join(" | ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
