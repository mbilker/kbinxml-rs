use std::net::Ipv4Addr;
use std::str::FromStr;

use failure::{Fail, ResultExt};

use crate::error::{KbinError, KbinErrorKind};

pub trait FromKbinString: Sized {
  fn from_kbin_string(input: &str) -> Result<Self, KbinError>;
}

fn space_check(input: &str) -> Result<(), KbinError> {
  // check for space character
  if input.find(' ').is_some() {
    return Err(KbinErrorKind::InvalidState.into());
  }

  Ok(())
}

fn parse_tuple<T>(node_type: &'static str, input: &str, output: &mut [T]) -> Result<(), KbinError>
  where T: FromStr,
        T::Err: Fail
{
  let count = input.split(' ').count();
  if count != output.len() {
    return Err(KbinErrorKind::SizeMismatch(node_type, output.len(), count).into());
  }

  for (i, part) in input.split(' ').enumerate() {
    output[i] = part.parse::<T>().context(KbinErrorKind::StringParse(node_type))?;
  }

  Ok(())
}

impl FromKbinString for bool {
  fn from_kbin_string(input: &str) -> Result<Self, KbinError> {
    match input {
      "0" => Ok(false),
      "1" => Ok(true),
      input => Err(KbinErrorKind::InvalidBooleanInput(u8::from_kbin_string(input)?).into()),
    }
  }
}

impl FromKbinString for Ipv4Addr {
  fn from_kbin_string(input: &str) -> Result<Self, KbinError> {
    space_check(input)?;

    let count = input.split('.').count();
    if count != 4 {
      return Err(KbinErrorKind::SizeMismatch("Ipv4Addr", 4, count).into());
    }

    let mut octets = [0; 4];

    // IP addresses are split by a period, so do not use `parse_tuple`
    for (i, part) in input.split('.').enumerate() {
      octets[i] = part.parse::<u8>().context(KbinErrorKind::StringParse("Ipv4Addr"))?;
    }

    Ok(Ipv4Addr::from(octets))
  }
}

macro_rules! basic_parse {
  (
    $($type:ty),*$(,)?
  ) => {
    $(
      impl FromKbinString for $type {
        fn from_kbin_string(input: &str) -> Result<Self, KbinError> {
          space_check(input)?;

          input.parse::<$type>().context(KbinErrorKind::StringParse(stringify!($type))).map_err(Into::into)
        }
      }
    )*
  };
}

macro_rules! tuple_parse {
  (
    bool: [$($bool_count:expr),*],
    multi: [
      $([$type:ident ; $($count:expr),*]),*$(,)?
    ]
  ) => {
    $(
      impl FromKbinString for [bool; $bool_count] {
        fn from_kbin_string(input: &str) -> Result<Self, KbinError> {
          const TYPE_NAME: &'static str = concat!("[bool; ", stringify!($bool_count), "]");

          let count = input.split(' ').count();
          if count != $bool_count {
            return Err(KbinErrorKind::SizeMismatch(TYPE_NAME, $bool_count, count).into());
          }

          let mut value = Self::default();

          for (i, part) in input.split(' ').enumerate() {
            value[i] = bool::from_kbin_string(part)?;
          }

          Ok(value)
        }
      }
    )*
    $(
      $(
        impl FromKbinString for [$type; $count] {
          fn from_kbin_string(input: &str) -> Result<Self, KbinError> {
            let mut value = Self::default();
            parse_tuple(concat!("[", stringify!($type), "; ", stringify!($count), "]"), input, &mut value)?;

            Ok(value)
          }
        }
      )*
    )*
  };
}

basic_parse! {
  i8, u8,
  i16, u16,
  i32, u32,
  i64, u64,
  f32, f64,
}

tuple_parse! {
  bool: [2, 3, 4, 16],
  multi: [
    [i8; 2, 3, 4, 16],
    [u8; 2, 3, 4, 16],
    [i16; 2, 3, 4, 8],
    [u16; 2, 3, 4, 8],
    [i32; 2, 3, 4],
    [u32; 2, 3, 4],
    [i64; 2, 3, 4],
    [u64; 2, 3, 4],
    [f32; 2, 3, 4],
    [f64; 2, 3, 4],
  ]
}
