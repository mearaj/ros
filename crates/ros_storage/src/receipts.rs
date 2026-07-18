//! Immutable receipt projections → ESC/POS bytes and minimal PDF.
//!
//! Hardware acceptance for Epson TM-T82X class printers remains a Stage 6
//! publication gate; these encoders are deterministic and unit-tested.

/// ESC/POS profile for 80 mm Epson TM-T82X class printers (ADR 0008).
pub fn encode_escpos_receipt(receipt_text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(receipt_text.len() + 32);
    // Initialize printer
    out.extend_from_slice(&[0x1b, b'@']);
    // Select left align
    out.extend_from_slice(&[0x1b, b'a', 0]);
    for line in receipt_text.lines() {
        out.extend_from_slice(line.as_bytes());
        out.push(b'\n');
    }
    // Feed and partial cut
    out.extend_from_slice(&[0x1d, b'V', 1]);
    out
}

/// Minimal single-page PDF containing the receipt as monospace text lines.
pub fn encode_simple_pdf_receipt(receipt_text: &str) -> Vec<u8> {
    let escaped = receipt_text
        .chars()
        .map(|ch| match ch {
            '\\' => "\\\\".to_owned(),
            '(' => "\\(".to_owned(),
            ')' => "\\)".to_owned(),
            '\r' => String::new(),
            '\n' => ") Tj T* (".to_owned(),
            c if c.is_control() => String::new(),
            c => c.to_string(),
        })
        .collect::<String>();
    let content = format!("BT /F1 9 Tf 36 800 Td 12 TL ({escaped}) Tj ET");
    let content_len = content.len();
    let objects = [
        "1 0 obj<< /Type /Catalog /Pages 2 0 R >>endobj\n".to_owned(),
        "2 0 obj<< /Type /Pages /Kids [3 0 R] /Count 1 >>endobj\n".to_owned(),
        "3 0 obj<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources<< /Font<< /F1 5 0 R >> >> >>endobj\n".to_owned(),
        format!("4 0 obj<< /Length {content_len} >>stream\n{content}\nendstream\nendobj\n"),
        "5 0 obj<< /Type /Font /Subtype /Type1 /BaseFont /Courier >>endobj\n".to_owned(),
    ];
    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets = Vec::with_capacity(objects.len());
    for object in &objects {
        offsets.push(pdf.len());
        pdf.push_str(object);
    }
    let xref_at = pdf.len();
    pdf.push_str(&format!("xref\n0 {}\n", objects.len() + 1));
    pdf.push_str("0000000000 65535 f \n");
    for offset in offsets {
        pdf.push_str(&format!("{offset:010} 00000 n \n"));
    }
    pdf.push_str(&format!(
        "trailer<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n",
        objects.len() + 1
    ));
    pdf.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escpos_contains_init_text_and_cut() {
        let bytes = encode_escpos_receipt("Hello\nWorld");
        assert!(bytes.starts_with(&[0x1b, b'@']));
        assert!(bytes.windows(5).any(|w| w == b"Hello"));
        assert!(bytes.ends_with(&[0x1d, b'V', 1]));
    }

    #[test]
    fn pdf_starts_with_header_and_contains_text() {
        let bytes = encode_simple_pdf_receipt("Invoice #1\nTotal 100");
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.starts_with("%PDF-1.4"));
        assert!(text.contains("Invoice #1"));
        assert!(text.contains("%%EOF"));
    }
}
