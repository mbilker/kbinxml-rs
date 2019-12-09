use std::error::Error;
use std::fmt;

use crate::{SIG_COMPRESSED, SIG_UNCOMPRESSED};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompressionType {
    Compressed,
    Uncompressed,
}

#[derive(Debug)]
pub struct UnknownCompression(u8);

impl CompressionType {
    pub fn from_byte(byte: u8) -> Result<Self, UnknownCompression> {
        match byte {
            SIG_COMPRESSED => Ok(CompressionType::Compressed),
            SIG_UNCOMPRESSED => Ok(CompressionType::Uncompressed),
            _ => Err(UnknownCompression(byte)),
        }
    }

    pub fn to_byte(&self) -> u8 {
        match *self {
            CompressionType::Compressed => SIG_COMPRESSED,
            CompressionType::Uncompressed => SIG_UNCOMPRESSED,
        }
    }
}

impl Default for CompressionType {
    fn default() -> Self {
        CompressionType::Compressed
    }
}

impl fmt::Display for UnknownCompression {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Unknown compression type: 0x{:x}", self.0)
    }
}

impl Error for UnknownCompression {}
