use std::error::Error;
use std::net::Ipv4Addr;
use std::str::FromStr;

use snafu::ResultExt;

use crate::error::*;

pub trait FromKbinString: Sized {
    fn from_kbin_string(input: &str) -> Result<Self>;
}

fn space_check(input: &str) -> Result<()> {
    // check for space character
    if input.find(' ').is_some() {
        return Err(KbinError::InvalidState.into());
    }

    Ok(())
}

fn parse_tuple<T>(node_type: &'static str, input: &str, output: &mut [T]) -> Result<()>
where
    T: FromStr,
    T::Err: Error + Send + Sync + 'static,
{
    let count = input.split(' ').count();
    if count != output.len() {
        return Err(KbinError::SizeMismatch {
            node_type,
            expected: output.len(),
            actual: count,
        });
    }

    for (i, part) in input.split(' ').enumerate() {
        output[i] = part
            .parse::<T>()
            .map_err(|e| Box::new(e) as Box<(dyn Error + Send + Sync + 'static)>)
            .context(StringParseSnafu { node_type })?;
    }

    Ok(())
}

impl FromKbinString for bool {
    fn from_kbin_string(input: &str) -> Result<Self> {
        match input {
            "false" | "0" => Ok(false),
            "true" | "1" => Ok(true),
            // Some text kbin XML files have values other than 0 or 1.
            input => u8::from_kbin_string(input).map(|v| v > 0),
        }
    }
}

impl FromKbinString for Ipv4Addr {
    fn from_kbin_string(input: &str) -> Result<Self> {
        space_check(input)?;

        let count = input.split('.').count();
        if count != 4 {
            return Err(KbinError::SizeMismatch {
                node_type: "Ipv4Addr",
                expected: 4,
                actual: count,
            });
        }

        let mut octets = [0; 4];

        // IP addresses are split by a period, so do not use `parse_tuple`
        for (i, part) in input.split('.').enumerate() {
            octets[i] = part.parse::<u8>().context(StringParseIntSnafu {
                node_type: "Ipv4Addr",
            })?;
        }

        Ok(Ipv4Addr::from(octets))
    }
}

macro_rules! basic_int_parse {
  (
    $($type:ty),*$(,)?
  ) => {
    $(
      impl FromKbinString for $type {
        fn from_kbin_string(input: &str) -> Result<Self> {
          space_check(input)?;

          if input.starts_with("0x") {
            <$type>::from_str_radix(&input[2..], 16)
              .context(StringParseIntSnafu { node_type: stringify!($type) })
          } else {
            input.parse::<$type>()
              .context(StringParseIntSnafu { node_type: stringify!($type) })
          }
        }
      }
    )*
  };
}

macro_rules! basic_float_parse {
  (
    $($type:ty),*$(,)?
  ) => {
    $(
      impl FromKbinString for $type {
        fn from_kbin_string(input: &str) -> Result<Self> {
          space_check(input)?;

          input.parse::<$type>()
            .context(StringParseFloatSnafu { node_type: stringify!($type) })
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
        fn from_kbin_string(input: &str) -> Result<Self> {
          const TYPE_NAME: &'static str = concat!("[bool; ", stringify!($bool_count), "]");

          let count = input.split(' ').count();
          if count != $bool_count {
            return Err(KbinError::SizeMismatch { node_type: TYPE_NAME, expected: $bool_count, actual: count });
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
          fn from_kbin_string(input: &str) -> Result<Self> {
            let mut value = Self::default();
            parse_tuple(concat!("[", stringify!($type), "; ", stringify!($count), "]"), input, &mut value)?;

            Ok(value)
          }
        }
      )*
    )*
  };
}

basic_int_parse! {
  i8, u8,
  i16, u16,
  i32, u32,
  i64, u64,
}

basic_float_parse! {
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
