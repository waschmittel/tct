//! Delivering a PDF to a browser session.
//!
//! In the Docker image `tct` runs on the server behind `ttyd`, so a file
//! written there is out of the user's reach. Instead the PDF travels through
//! the terminal stream as a private OSC sequence that `docker/pwa/tct-pwa.js`
//! turns into a download:
//!
//! ```text
//! ESC ] 7733 ; <index> ; <total> ; <base64 file name> ; <base64 chunk> BEL
//! ```
//!
//! Chunks keep every sequence small; the receiver restarts when it sees
//! index 0. Keep the wire format in step with `tct-pwa.js` — both sides pin it
//! with the same literal in their tests.

use std::io::Write;

use anyhow::bail;

use crate::app::base64_encode;

pub const OSC_CODE: u16 = 7733;
/// A multiple of three so only the final chunk carries base64 padding.
const CHUNK_BYTES: usize = 24 * 1024;
pub const MAX_PDF_BYTES: usize = 25 * 1024 * 1024;

pub fn send(out: &mut dyn Write, file_name: &str, pdf: &[u8]) -> anyhow::Result<()> {
    if pdf.is_empty() {
        bail!("nothing to send");
    }
    if pdf.len() > MAX_PDF_BYTES {
        bail!("PDF is too large to send ({} MB)", pdf.len() / (1024 * 1024));
    }
    let name = base64_encode(file_name.as_bytes());
    let total = pdf.len().div_ceil(CHUNK_BYTES).max(1);
    for (index, chunk) in pdf.chunks(CHUNK_BYTES).enumerate() {
        write!(out, "\x1b]{OSC_CODE};{index};{total};{name};{}\x07", base64_encode(chunk))?;
    }
    out.flush()?;
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;

    /// A writer whose contents stay readable after it moved into an `App`.
    #[derive(Clone, Default)]
    pub struct SharedBuf(pub Rc<RefCell<Vec<u8>>>);

    impl Write for SharedBuf {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn decode(text: &str) -> Vec<u8> {
        const TABLE: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = Vec::new();
        let mut bits = 0u32;
        let mut have = 0;
        for c in text.trim_end_matches('=').chars() {
            bits = (bits << 6) | TABLE.find(c).expect("base64 character") as u32;
            have += 6;
            if have >= 8 {
                have -= 8;
                out.push((bits >> have) as u8);
                bits &= (1 << have) - 1;
            }
        }
        out
    }

    /// Split a stream of sequences into (index, total, name, bytes).
    pub fn parse(stream: &[u8]) -> Vec<(usize, usize, String, Vec<u8>)> {
        let text = String::from_utf8(stream.to_vec()).unwrap();
        text.split('\x07')
            .filter(|s| !s.is_empty())
            .map(|seq| {
                let body = seq.strip_prefix("\x1b]7733;").expect("OSC 7733 prefix");
                let parts: Vec<&str> = body.split(';').collect();
                assert_eq!(parts.len(), 4, "{body}");
                (
                    parts[0].parse().unwrap(),
                    parts[1].parse().unwrap(),
                    String::from_utf8(decode(parts[2])).unwrap(),
                    decode(parts[3]),
                )
            })
            .collect()
    }

    #[test]
    fn the_wire_format_is_pinned_for_the_browser_side() {
        let mut out = Vec::new();
        send(&mut out, "a b.pdf", b"%PDF-1.7\n").unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "\x1b]7733;0;1;YSBiLnBkZg==;JVBERi0xLjcK\x07");
    }

    #[test]
    fn large_pdfs_are_split_into_ordered_chunks_that_reassemble() {
        let pdf: Vec<u8> = (0..60_000u32).map(|i| (i % 251) as u8).collect();
        let mut out = Vec::new();
        send(&mut out, "big.pdf", &pdf).unwrap();
        let parts = parse(&out);
        assert_eq!(parts.len(), 3);
        for (i, (index, total, name, _)) in parts.iter().enumerate() {
            assert_eq!((*index, *total, name.as_str()), (i, 3, "big.pdf"));
        }
        let joined: Vec<u8> = parts.into_iter().flat_map(|p| p.3).collect();
        assert_eq!(joined, pdf);
    }

    #[test]
    fn an_empty_pdf_is_refused() {
        let mut out = Vec::new();
        assert!(send(&mut out, "x.pdf", b"").is_err());
        assert!(out.is_empty());
    }

    #[test]
    fn oversized_pdfs_are_refused_without_writing() {
        let mut out = Vec::new();
        let err = send(&mut out, "x.pdf", &vec![0u8; MAX_PDF_BYTES + 1]).unwrap_err();
        assert!(err.to_string().contains("too large"));
        assert!(out.is_empty());
    }
}
