use error::{KbinError, KbinErrorKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compression {
  Compressed,
  Uncompressed,
}

impl Compression {
  pub fn from_byte(byte: u8) -> Result<Self, KbinError> {
    match byte {
      0x42 => Ok(Compression::Compressed),
      0x45 => Ok(Compression::Uncompressed),
      _ => Err(KbinErrorKind::UnknownCompression.into()),
    }
  }

  pub fn _to_byte(&self) -> u8 {
    match *self {
      Compression::Compressed   => 0x42,
      Compression::Uncompressed => 0x45,
    }
  }
}
