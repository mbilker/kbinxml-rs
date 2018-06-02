use std::fmt;

use failure::{Backtrace, Context, Fail};

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
