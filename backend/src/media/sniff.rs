//! What an uploaded file is, from its first bytes; the name and the Content-Type header are
//! only hints. One place for every upload that takes a scan or a photo, so a new kind of
//! file is allowed (or refused) everywhere at once.

/// A scan or a photo: PDF, JPEG, PNG or WebP. Doctor's notes, employee documents.
pub fn scan_or_photo(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    if bytes.starts_with(b"%PDF") {
        Some(("application/pdf", "pdf"))
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(("image/jpeg", "jpg"))
    } else if bytes.starts_with(b"\x89PNG") {
        Some(("image/png", "png"))
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(("image/webp", "webp"))
    } else {
        None
    }
}

/// A supplier invoice: a scan or a photo, or the NAV online invoice XML as suppliers'
/// systems send it. Only here: XML anywhere else is a file a browser might run.
pub fn incoming_invoice(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    scan_or_photo(bytes).or_else(|| {
        let start = bytes.trim_ascii_start();
        start
            .starts_with(b"<")
            .then_some(("application/xml", "xml"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_told_apart_by_their_first_bytes() {
        assert_eq!(incoming_invoice(b"%PDF-1.7 ...").map(|s| s.1), Some("pdf"));
        assert_eq!(
            incoming_invoice(&[0xFF, 0xD8, 0xFF, 0xE0]).map(|s| s.1),
            Some("jpg")
        );
        assert_eq!(incoming_invoice(b"\x89PNG\r\n").map(|s| s.1), Some("png"));
        assert_eq!(
            incoming_invoice(b"  <?xml version=\"1.0\"?>").map(|s| s.1),
            Some("xml")
        );
        assert_eq!(incoming_invoice(b"MZ\x90\x00"), None);
    }

    #[test]
    fn a_scan_is_never_xml() {
        assert_eq!(scan_or_photo(b"%PDF-1.4").map(|s| s.1), Some("pdf"));
        assert_eq!(
            scan_or_photo(b"RIFF\0\0\0\0WEBPVP8 ").map(|s| s.1),
            Some("webp")
        );
        assert_eq!(scan_or_photo(b"<?xml version=\"1.0\"?>"), None);
        assert_eq!(scan_or_photo(b"<html><script>"), None);
    }
}
