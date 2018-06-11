use std::fmt;
use std::string::FromUtf8Error;

use failure::{Backtrace, Context, Fail};

use node_types::KbinType;

#[derive(Debug)]
pub struct KbinError {
  inner: Context<KbinErrorKind>,
}

#[derive(Debug, Fail)]
pub enum KbinErrorKind {
  #[fail(display = "Unable to read data")]
  DataRead,

  #[fail(display = "Unable to read data size")]
  DataReadSize,

  #[fail(display = "Unable to read 1 byte data")]
  DataReadOneByte,

  #[fail(display = "Unable to read 2 byte data")]
  DataReadTwoByte,

  #[fail(display = "Unable to read aligned data from data buffer")]
  DataReadAligned,

  #[fail(display = "Unable to seek data buffer")]
  Seek,

  #[fail(display = "Unable to read signature byte")]
  SignatureRead,

  #[fail(display = "Unable to read compression byte")]
  CompressionRead,

  #[fail(display = "Unknown compression value")]
  UnknownCompression,

  #[fail(display = "Unable to read encoding byte")]
  EncodingRead,

  #[fail(display = "Unable to read encoding negation byte")]
  EncodingNegationRead,

  #[fail(display = "Unable to read len_node")]
  LenNodeRead,

  #[fail(display = "Unable to read len_data")]
  LenDataRead,

  #[fail(display = "Unable to read node type")]
  NodeTypeRead,

  #[fail(display = "Unable to read binary/string byte length")]
  BinaryLengthRead,

  #[fail(display = "Unable to read array node length")]
  ArrayLengthRead,

  #[fail(display = "Failed to write {} to output string", _0)]
  ByteParse(&'static str),

  #[fail(display = "Unable to read sixbit string length")]
  SixbitLengthRead,

  #[fail(display = "Unable to read sixbit string content")]
  SixbitRead,

  #[fail(display = "Unable to write sixbit string length")]
  SixbitLengthWrite,

  #[fail(display = "Unable to write sixbit string content")]
  SixbitWrite,

  #[fail(display = "Unable to interpret string as UTF-8")]
  Utf8,

  #[fail(display = "Unknown encoding")]
  UnknownEncoding,

  #[fail(display = "Unable to interpret string as alternate encoding")]
  Encoding,

  #[fail(display = "Unable to write {} header field", _0)]
  HeaderWrite(&'static str),

  #[fail(display = "Unable to write a {}", _0)]
  DataWrite(&'static str),

  #[fail(display = "Size Mismatch, type: {}, expected size: {}, actual size: {}", _0, _1, _2)]
  SizeMismatch(KbinType, usize, usize),

  #[fail(display = "Unable to interpret input as {}", _0)]
  StringParse(&'static str),

  #[fail(display = "Unable to convert from hexadecimal")]
  HexError,
}

impl fmt::Display for KbinError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    fmt::Display::fmt(&self.inner, f)
  }
}

impl Fail for KbinError {
  fn cause(&self) -> Option<&Fail> {
    self.inner.cause()
  }

  fn backtrace(&self) -> Option<&Backtrace> {
    self.inner.backtrace()
  }
}

impl From<KbinErrorKind> for KbinError {
  fn from(kind: KbinErrorKind) -> KbinError {
    KbinError { inner: Context::new(kind) }
  }
}

impl From<Context<KbinErrorKind>> for KbinError {
  fn from(inner: Context<KbinErrorKind>) -> KbinError {
    KbinError { inner }
  }
}

impl From<FromUtf8Error> for KbinError {
  fn from(inner: FromUtf8Error) -> KbinError {
    inner.context(KbinErrorKind::Utf8).into()
  }
}
