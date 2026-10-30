//! `tl-core-reference`: a byte-exact Rust port of the TradeLayer C++ payload layer and activation
//! rules, used as a differential oracle. It is NOT a node: it contains no consensus state
//! machine beyond the activation table; the full Rust consensus port lives in
//! `patrickdugan/TradeLayer-Rust`.
//!
//! * [`varint`]: `CompressInteger` / `DecompressInteger` (`src/tradelayer/varint.cpp`).
//! * [`payload`]: [`Payload`], [`encode`], [`decode`] (`createpayload.cpp` + `tx.cpp` parsing).
//! * [`types`]: the transaction type table (`tradelayer.h`) and dispatch facts.
//! * [`rules`]: feature ids, per-network activation heights, activation/deactivation
//!   (`rules.cpp`, `rules.h`, `activation.cpp`).
//! * [`text`]: [`Str255`], the 255-byte bounded string.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod payload;
pub mod rules;
pub mod text;
pub mod types;
pub mod varint;

pub use payload::{decode, encode, try_encode, DecodeError, EncodeError, Payload, CREATABLE, MAX_PAYLOAD_LEN};
pub use text::{Str255, TextError, MAX_STR_LEN};

/// Lower-case hex of `bytes`.
pub fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Parses an even-length lower- or upper-case hex string.
pub fn from_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) || !s.is_ascii() {
        return None;
    }
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()).collect()
}
