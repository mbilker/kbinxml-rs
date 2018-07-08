use std::error::Error as StdError;
use std::fmt::{self, Display};

use failure::{Compat, Context, Fail};
use serde::{de, ser};

use error::{KbinError, KbinErrorKind};

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
