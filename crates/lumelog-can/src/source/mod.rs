//! Streaming CAN-log sources (spec §7.3). Each source turns a line iterator
//! `((line_number, text))` into `Frame`s, skipping and counting malformed lines
//! without ever panicking or stalling the stream.

pub mod asc;
pub mod candump;
pub mod regex_source;

use crate::frame::{Frame, SourceStats};

/// One source line paired with its 1-based number in the underlying file.
pub type Line = (u64, String);

/// A frame-emitting iterator over some CAN log dialect.
pub trait FrameSource {
    /// Next successfully decoded frame; internal skips advance past bad lines.
    fn next_frame(&mut self) -> Option<Frame>;
    /// Tallies of frames emitted vs. non-blank lines skipped so far.
    fn stats(&self) -> SourceStats;
}

/// Parse an even-length hex string into bytes. `None` on odd length or a non-hex
/// character; empty input is an empty payload. Shared by every source.
pub(crate) fn hex_to_bytes(s: &str) -> Option<Vec<u8>> {
    if s.is_empty() {
        return Some(Vec::new());
    }
    // Reject non-ASCII before slicing by byte offset: `s[i..i + 2]` panics when
    // a multi-byte char straddles that boundary (e.g. an emoji payload in a
    // candump line). A malformed payload is skipped and counted by the caller,
    // never an error frame or a crash.
    if !s.is_ascii() || !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_to_bytes_rejects_non_ascii_without_panicking() {
        // A 4-byte emoji has even UTF-8 length; slicing at byte 2 would panic.
        assert_eq!(hex_to_bytes("😀"), None);
        assert_eq!(hex_to_bytes("AB😀"), None);
        assert_eq!(hex_to_bytes("éé"), None, "even-length non-ASCII still rejected");
        // Valid ASCII hex and empty payload keep their old behavior.
        assert_eq!(
            hex_to_bytes("DEADBEEF"),
            Some(vec![0xDE, 0xAD, 0xBE, 0xEF])
        );
        assert_eq!(hex_to_bytes(""), Some(Vec::new()));
        assert_eq!(hex_to_bytes("ABC"), None, "odd length still rejected");
        assert_eq!(hex_to_bytes("ZZ"), None, "non-hex ASCII still rejected");
    }

    #[test]
    fn candump_non_ascii_payload_is_skipped_not_an_error_frame() {
        let lines = vec![
            (1u64, "can0 123#😀".to_string()),
            (2u64, "can0 123#DEADBEEF".to_string()),
        ];
        let mut src = candump::CandumpSource::new(lines.into_iter());
        // The malformed line is skipped; the following valid frame still decodes.
        let frame = src.next_frame().expect("valid frame after bad line");
        assert_eq!(frame.line_number, 2);
        assert_eq!(frame.dlc, 4);
        assert!(src.next_frame().is_none());
        let stats = src.stats();
        assert_eq!(stats.decoded, 1);
        assert_eq!(stats.skipped, 1, "malformed line counted, not silently dropped");
    }
}

pub use asc::AscSource;
pub use candump::CandumpSource;
pub use regex_source::RegexSource;
