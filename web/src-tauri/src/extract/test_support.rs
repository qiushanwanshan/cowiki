use std::io::Write;
use std::path::Path;

pub fn write_minimal_xlsx(path: &Path, sheet_name: &str, rows: &[[&str; 2]]) {
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(br#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#).unwrap();

    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#).unwrap();

    zip.start_file("xl/_rels/workbook.xml.rels", options)
        .unwrap();
    zip.write_all(br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#).unwrap();

    zip.start_file("xl/workbook.xml", options).unwrap();
    let workbook_xml = format!(
        r#"<?xml version="1.0"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheets><sheet name="{sheet_name}" sheetId="1" r:id="rId1" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"/></sheets></workbook>"#
    );
    zip.write_all(workbook_xml.as_bytes()).unwrap();

    zip.start_file("xl/worksheets/sheet1.xml", options).unwrap();
    let mut sheet_data = String::from(
        r#"<?xml version="1.0"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>"#,
    );
    for (row_index, row) in rows.iter().enumerate() {
        let row_number = row_index + 1;
        sheet_data.push_str(&format!(r#"<row r="{row_number}">"#));
        for (col_index, value) in row.iter().enumerate() {
            let column = (b'A' + col_index as u8) as char;
            sheet_data.push_str(&format!(
                r#"<c r="{column}{row_number}" t="inlineStr"><is><t>{value}</t></is></c>"#
            ));
        }
        sheet_data.push_str("</row>");
    }
    sheet_data.push_str("</sheetData></worksheet>");
    zip.write_all(sheet_data.as_bytes()).unwrap();

    zip.finish().unwrap();
}

pub fn write_minimal_pptx(path: &Path, slide_titles: &[&str]) {
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    for (index, title) in slide_titles.iter().enumerate() {
        let slide_number = index + 1;
        zip.start_file(format!("ppt/slides/slide{slide_number}.xml"), options)
            .unwrap();
        let slide_xml = format!(
            r#"<?xml version="1.0"?><p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>{title}</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>"#
        );
        zip.write_all(slide_xml.as_bytes()).unwrap();
    }

    zip.finish().unwrap();
}

pub fn write_minimal_odf_zip(path: &Path, content_xml: &str) {
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("content.xml", options).unwrap();
    zip.write_all(content_xml.as_bytes()).unwrap();
    zip.finish().unwrap();
}
