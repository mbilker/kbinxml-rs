use std::error::Error as StdError;
use std::fmt::{self, Display};
use std::result::Result as StdResult;
use std::string::FromUtf8Error;

use failure::{Backtrace, Compat, Context, Fail};
use serde::{de, ser};

use node_types::KbinType;

pub type Result<T> = StdResult<T, KbinError>;

#[derive(Debug)]
pub struct KbinError {
  inner: Context<KbinErrorKind>,
}

#[derive(Clone, Debug, Fail)]
pub enum KbinErrorKind {
  #[fail(display = "Unable to read {} byte from header", _0)]
  HeaderRead(&'static str),

  #[fail(display = "Unable to write {} header field", _0)]
  HeaderWrite(&'static str),

  #[fail(display = "Invalid byte value for {} header field", _0)]
  HeaderValue(&'static str),

  #[fail(display = "Unable to read {} bytes from data buffer", _0)]
  DataRead(usize),

  #[fail(display = "Unable to write a {} to data buffer", _0)]
  DataWrite(&'static str),

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

  #[fail(display = "Unknown compression value")]
  UnknownCompression,

  #[fail(display = "Unknown encoding")]
  UnknownEncoding,

  #[fail(display = "Unable to interpret string as alternate encoding")]
  Encoding,

  #[fail(display = "Size Mismatch, type: {}, expected size: {}, actual size: {}", _0, _1, _2)]
  SizeMismatch(KbinType, usize, usize),

  #[fail(display = "Unable to interpret input as {}", _0)]
  StringParse(&'static str),

  #[fail(display = "Unable to convert from hexadecimal")]
  HexError,

  #[fail(display = "Missing type hint where one is required")]
  MissingTypeHint,

  #[fail(display = "Type mismatch, expected: {}, found: {}", _0, _1)]
  TypeMismatch(KbinType, KbinType),
}

impl fmt::Display for KbinError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    fmt::Display::fmt(&self.inner, f)
  }
}

impl Clone for KbinError {
  fn clone(&self) -> Self {
    Self {
      inner: Context::new(self.inner.get_context().clone())
    }
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
  fn from(kind: KbinErrorKind) -> Self {
    Self { inner: Context::new(kind) }
  }
}

impl From<Context<KbinErrorKind>> for KbinError {
  fn from(inner: Context<KbinErrorKind>) -> Self {
    Self { inner }
  }
}

impl From<FromUtf8Error> for KbinError {
  fn from(inner: FromUtf8Error) -> KbinError {
    inner.context(KbinErrorKind::Utf8).into()
  }
}

#[derive(Clone, Debug)]
pub enum Error {
  Message(String),

  Wrapped(Compat<KbinError>),
}

impl ser::Error for Error {
  fn custom<T: Display>(msg: T) -> Self {
    Error::Message(msg.to_string())
  }
}

impl de::Error for Error {
  fn custom<T: Display>(msg: T) -> Self {
    Error::Message(msg.to_string())
  }
}

impl Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.write_str(StdError::description(self))
  }
}

impl StdError for Error {
  fn description(&self) -> &str {
    match *self {
      Error::Message(ref msg) => msg,
      Error::Wrapped(ref err) => err.description(),
    }
  }
}

impl From<KbinError> for Error {
  fn from(inner: KbinError) -> Self {
    Error::Wrapped(inner.compat())
  }
}

impl From<KbinErrorKind> for Error {
  fn from(inner: KbinErrorKind) -> Self {
    Error::Wrapped(KbinError::from(inner).compat())
  }
}

impl From<Context<KbinErrorKind>> for Error {
  fn from(inner: Context<KbinErrorKind>) -> Self {
    Error::Wrapped(KbinError::from(inner).compat())
  }
}
