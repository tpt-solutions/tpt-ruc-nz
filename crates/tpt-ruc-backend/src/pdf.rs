// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Plain-text table reports as PDF, using `lopdf`.
//!
//! The report is a title, a subtitle and a fixed-width table. Courier is a
//! monospaced font, so the columns line up without measuring glyphs. Only the
//! standard built-in font is used, so the PDF needs no embedded fonts.

use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Document, Object, Stream};

/// Lines that fit on one A4 page at the sizes used below.
const LINES_PER_PAGE: usize = 52;
const PAGE_WIDTH: i64 = 595;
const PAGE_HEIGHT: i64 = 842;
const MARGIN: i64 = 40;
const LINE_HEIGHT: i64 = 13;

/// Builds a PDF with `title`, `subtitle`, a bold `header` row and one line per entry in `lines`.
pub fn table_document(
    title: &str,
    subtitle: &str,
    header: &str,
    lines: &[String],
) -> Result<Vec<u8>, lopdf::Error> {
    let mut document = Document::with_version("1.5");
    let pages_id = document.new_object_id();
    let font_id = document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Courier",
    });
    let bold_id = document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Courier-Bold",
    });
    let resources_id = document.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_id, "F2" => bold_id },
    });

    let chunks: Vec<&[String]> = if lines.is_empty() {
        vec![&[]]
    } else {
        lines.chunks(LINES_PER_PAGE).collect()
    };
    let page_count = chunks.len();
    let mut page_ids = Vec::with_capacity(page_count);

    for chunk in &chunks {
        let mut operations = vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F2".into(), 13.into()]),
            Operation::new("Td", vec![MARGIN.into(), (PAGE_HEIGHT - MARGIN).into()]),
            Operation::new("Tj", vec![Object::string_literal(title)]),
            Operation::new("Tf", vec!["F1".into(), 9.into()]),
            Operation::new("Td", vec![0.into(), (-LINE_HEIGHT * 2).into()]),
            Operation::new("Tj", vec![Object::string_literal(subtitle)]),
            Operation::new("Td", vec![0.into(), (-LINE_HEIGHT * 2).into()]),
            Operation::new("Tf", vec!["F2".into(), 9.into()]),
            Operation::new("Tj", vec![Object::string_literal(header)]),
            Operation::new("Tf", vec!["F1".into(), 9.into()]),
        ];
        for line in chunk.iter() {
            operations.push(Operation::new("Td", vec![0.into(), (-LINE_HEIGHT).into()]));
            operations.push(Operation::new(
                "Tj",
                vec![Object::string_literal(line.as_str())],
            ));
        }
        operations.push(Operation::new("ET", vec![]));
        let content = Content { operations };
        let content_id = document.add_object(Stream::new(dictionary! {}, content.encode()?));
        let page_id = document.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), PAGE_WIDTH.into(), PAGE_HEIGHT.into()],
        });
        page_ids.push(page_id);
    }

    let kids: Vec<Object> = page_ids.iter().map(|id| (*id).into()).collect();
    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => page_count as i64,
        }),
    );
    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_is_a_pdf_with_the_expected_page_count() {
        let lines: Vec<String> = (0..120).map(|i| format!("line {i}")).collect();
        let bytes = table_document("Title", "Sub", "a  b  c", &lines).unwrap();
        assert!(bytes.starts_with(b"%PDF-1.5"));
        let text = String::from_utf8_lossy(&bytes);
        assert!(
            text.contains("/Count 3"),
            "120 lines need three pages of 52"
        );
        assert!(text.contains("/Courier"));
    }

    #[test]
    fn parentheses_in_text_are_escaped() {
        let bytes = table_document("T (x)", "S", "H", &["a)b(c".to_owned()]).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(
            text.contains(r"a\)b\(c"),
            "parentheses must be escaped in the content stream"
        );
    }

    #[test]
    fn an_empty_report_still_has_one_page() {
        let bytes = table_document("Title", "Sub", "header", &[]).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("/Count 1"));
    }
}
