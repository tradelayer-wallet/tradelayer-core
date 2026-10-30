//! Bounded string fields.
//!
//! Every NUL-terminated string field in the C++ payload builders is cut to 255 bytes first
//! (`if (name.size() > 255) name = name.substr(0,255);`), and the node copies at most 255 bytes
//! into a `char[256]` field when parsing. [`Str255`] carries that bound in its type.

/// Maximum string length in bytes (excluding the NUL terminator).
pub const MAX_STR_LEN: usize = 255;

/// Errors constructing a [`Str255`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextError {
    /// More than 255 bytes.
    TooLong(usize),
    /// Contains a NUL byte, which would end the field early on the wire.
    EmbeddedNul,
}

/// A byte string of at most 255 bytes without NUL bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Str255(Vec<u8>);

impl Str255 {
    /// Strict constructor: rejects strings longer than 255 bytes and strings containing NUL.
    pub fn new(bytes: &[u8]) -> Result<Self, TextError> {
        if bytes.len() > MAX_STR_LEN {
            return Err(TextError::TooLong(bytes.len()));
        }
        Self::checked(bytes)
    }

    /// C++-compatible constructor: cuts the input to its first 255 bytes (the behaviour of the
    /// payload builders) and rejects embedded NUL bytes.
    pub fn truncating(bytes: &[u8]) -> Result<Self, TextError> {
        Self::checked(&bytes[..bytes.len().min(MAX_STR_LEN)])
    }

    fn checked(bytes: &[u8]) -> Result<Self, TextError> {
        if bytes.contains(&0) {
            return Err(TextError::EmbeddedNul);
        }
        Ok(Self(bytes.to_vec()))
    }

    /// The raw bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Length in bytes.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the string is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl TryFrom<&str> for Str255 {
    type Error = TextError;
    fn try_from(s: &str) -> Result<Self, TextError> {
        Self::new(s.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds() {
        assert!(Str255::new(&[b'a'; 255]).is_ok());
        assert_eq!(Str255::new(&[b'a'; 256]), Err(TextError::TooLong(256)));
        assert_eq!(Str255::truncating(&[b'a'; 300]).unwrap().len(), 255);
        assert_eq!(Str255::new(b"a\0b"), Err(TextError::EmbeddedNul));
        // The NUL is only rejected if it survives the cut.
        let mut s = vec![b'a'; 255];
        s.push(0);
        assert_eq!(Str255::truncating(&s).unwrap().len(), 255);
    }
}
