// src/barcode.rs
pub fn normalize(raw: &str) -> String {
    // 1) remove leading/trailing whitespace & control chars (CR/LF/TAB, BOM)
    let s = raw.trim_matches(|c: char| c.is_whitespace() || c == '\u{FEFF}');

    // 2) If your barcodes are numeric (EAN-13, etc.) keep only digits.
    //    If you have alphanumeric codes, change this to `retain(|c| !c.is_control())`.
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch.is_ascii_digit() {
            out.push(ch);
        }
    }
    out
}
