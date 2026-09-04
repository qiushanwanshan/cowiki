use std::path::Path;

use calamine::{open_workbook_auto, Data, Reader as _, SheetVisible};

use super::output::AdapterOutput;
use crate::extract::contract::{ExtractStats, SpreadsheetStats};

/// Renders every sheet as a Markdown table under its own heading.
pub fn extract_spreadsheet(path: &Path) -> Result<AdapterOutput, String> {
    let mut workbook =
        open_workbook_auto(path).map_err(|error| format!("cannot open spreadsheet: {error}"))?;
    let metadata = workbook.sheets_metadata().to_vec();
    let mut formula_count = 0_u32;
    for sheet in &metadata {
        if let Ok(formulas) = workbook.worksheet_formula(&sheet.name) {
            formula_count += formulas
                .rows()
                .flat_map(|row| row.iter())
                .filter(|cell| !cell.is_empty())
                .count() as u32;
        }
    }

    let mut sections = Vec::new();
    let mut non_empty_cell_count = 0_u32;
    let mut rendered_sheets = 0_u32;
    let worksheets = workbook.worksheets();
    let sheet_count = if metadata.is_empty() {
        worksheets.len() as u32
    } else {
        metadata.len() as u32
    };
    let visible_sheet_count = if metadata.is_empty() {
        sheet_count
    } else {
        metadata
            .iter()
            .filter(|sheet| sheet.visible == SheetVisible::Visible)
            .count() as u32
    };
    for (name, range) in worksheets {
        non_empty_cell_count += range
            .rows()
            .flat_map(|row| row.iter())
            .filter(|cell| !matches!(cell, Data::Empty))
            .count() as u32;
        if range.is_empty() {
            continue;
        }
        let mut rows = range.rows().map(spreadsheet_row_to_markdown);
        let Some(header) = rows.next() else {
            continue;
        };
        let column_count = range.get_size().1.max(1);
        let separator = format!("|{}", " --- |".repeat(column_count));
        let mut table = vec![header, separator];
        table.extend(rows);
        sections.push(format!("## {name}\n\n{}", table.join("\n")));
        rendered_sheets += 1;
    }

    let spreadsheet = SpreadsheetStats {
        sheet_count,
        visible_sheet_count,
        non_empty_cell_count,
        formula_count,
    };
    let mut stats = ExtractStats {
        table_count: rendered_sheets as usize,
        heading_count: rendered_sheets as usize,
        spreadsheet: Some(spreadsheet),
        ..ExtractStats::default()
    };
    stats.set_coverage(rendered_sheets, visible_sheet_count.max(sheet_count));
    Ok(AdapterOutput::with_structure(sections.join("\n\n"), stats))
}

pub(crate) fn spreadsheet_row_to_markdown(row: &[Data]) -> String {
    let cells = row
        .iter()
        .map(|cell| match cell {
            Data::Empty => String::new(),
            other => other
                .to_string()
                .replace("\r\n", "<br>")
                .replace(['\r', '\n'], "<br>")
                .replace('|', "\\|"),
        })
        .collect::<Vec<_>>()
        .join(" | ");
    format!("| {cells} |")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spreadsheet_cells_stay_inside_their_markdown_row() {
        let row = spreadsheet_row_to_markdown(&[Data::String("line one\nline two | value".into())]);
        assert_eq!(row, "| line one<br>line two \\| value |");
    }
}
