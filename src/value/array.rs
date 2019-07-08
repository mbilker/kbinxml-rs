//use std::convert::TryFrom;
use std::fmt;
use std::io::Cursor;
use std::net::Ipv4Addr;
//use std::str::FromStr;

//use rustc_hex::FromHex;

use crate::error::{KbinError, KbinErrorKind};
use crate::node_types::StandardType;
use crate::types::{FromKbinBytes, IntoKbinBytes};
use crate::types::FromKbinString;

#[derive(Clone, Debug, PartialEq)]
pub enum ValueArray {
  S8(Vec<i8>),
  U8(Vec<u8>),
  S16(Vec<i16>),
  U16(Vec<u16>),
  S32(Vec<i32>),
  U32(Vec<u32>),
  S64(Vec<i64>),
  U64(Vec<u64>),
  Ip4(Vec<Ipv4Addr>),
  Float(Vec<f32>),
  Double(Vec<f64>),
  S8_2(Vec<[i8; 2]>),
  U8_2(Vec<[u8; 2]>),
  S16_2(Vec<[i16; 2]>),
  U16_2(Vec<[u16; 2]>),
  S32_2(Vec<[i32; 2]>),
  U32_2(Vec<[u32; 2]>),
  S64_2(Vec<[i64; 2]>),
  U64_2(Vec<[u64; 2]>),
  Float2(Vec<[f32; 2]>),
  Double2(Vec<[f64; 2]>),
  S8_3(Vec<[i8; 3]>),
  U8_3(Vec<[u8; 3]>),
  S16_3(Vec<[i16; 3]>),
  U16_3(Vec<[u16; 3]>),
  S32_3(Vec<[i32; 3]>),
  U32_3(Vec<[u32; 3]>),
  S64_3(Vec<[i64; 3]>),
  U64_3(Vec<[u64; 3]>),
  Float3(Vec<[f32; 3]>),
  Double3(Vec<[f64; 3]>),
  S8_4(Vec<[i8; 4]>),
  U8_4(Vec<[u8; 4]>),
  S16_4(Vec<[i16; 4]>),
  U16_4(Vec<[u16; 4]>),
  S32_4(Vec<[i32; 4]>),
  U32_4(Vec<[u32; 4]>),
  S64_4(Vec<[i64; 4]>),
  U64_4(Vec<[u64; 4]>),
  Float4(Vec<[f32; 4]>),
  Double4(Vec<[f64; 4]>),
  Vs8(Vec<[i8; 16]>),
  Vu8(Vec<[u8; 16]>),
  Vs16(Vec<[i16; 8]>),
  Vu16(Vec<[u16; 8]>),
  Boolean(Vec<bool>),
  Boolean2(Vec<[bool; 2]>),
  Boolean3(Vec<[bool; 3]>),
  Boolean4(Vec<[bool; 4]>),
  Vb(Vec<[bool; 16]>),
}

macro_rules! type_impl {
  (
    $($konst:ident),*$(,)?
  ) => {
    pub fn from_standard_type(node_type: StandardType, input: &[u8]) -> Result<Option<Self>, KbinError> {
      let node_size = node_type.size * node_type.count;
      let len = input.len() / node_size;

      // Prevent reading incomplete input data
      if node_size * len != input.len() {
        return Err(KbinErrorKind::SizeMismatch(node_type.name, node_size, input.len()).into());
      }

      let mut reader = Cursor::new(input);

      let value = match node_type {
        StandardType::NodeStart |
        StandardType::NodeEnd |
        StandardType::FileEnd |
        StandardType::Attribute |
        StandardType::Binary |
        StandardType::String |
        StandardType::Time => return Ok(None),
        $(
          StandardType::$konst => {
            let mut values = Vec::with_capacity(len);

            for _ in 0..len {
              values.push(FromKbinBytes::from_kbin_bytes(&mut reader)?);
            }

            ValueArray::$konst(values)
          },
        )*
      };

      Ok(Some(value))
    }

    pub(super) fn from_string(node_type: StandardType, count: usize, input: &str, arr_count: usize) -> Result<Self, KbinError> {
      trace!("from_string(count: {}, input: {:?}, arr_count: {})", count, input, arr_count);

      // counter of the number of space characters encountered
      let mut i = 0;

      let iter = input.split(|c| {
        if c == ' ' {
          // increment ths space counter
          i += 1;

          // if the space counter is equal to count, then split
          let res = i == count;

          // if splitting, then reset the counter
          if res {
            i = 0;
          }

          res
        } else {
          false
        }
      });

      let value = match node_type {
        StandardType::NodeStart |
        StandardType::NodeEnd |
        StandardType::FileEnd |
        StandardType::Attribute |
        StandardType::Binary |
        StandardType::String |
        StandardType::Time => return Err(KbinErrorKind::InvalidState.into()),
        $(
          StandardType::$konst => {
            let mut values = Vec::new();

            for part in iter {
              values.push(FromKbinString::from_kbin_string(part)?);
            }

            ValueArray::$konst(values)
          },
        )*
      };

      Ok(value)
    }

    pub fn to_bytes_into(&self, output: &mut Vec<u8>) -> Result<(), KbinError> {
      let node_size = self.standard_type().size;

      match self {
        $(
          ValueArray::$konst(values) => {
            output.reserve(values.len() * node_size);
            for value in values {
              value.write_kbin_bytes(output);
            }
          },
        )*
      };

      Ok(())
    }

    pub fn standard_type(&self) -> StandardType {
      match self {
        $(
          ValueArray::$konst(_) => StandardType::$konst,
        )*
      }
    }

    pub fn len(&self) -> usize {
      match self {
        $(
          ValueArray::$konst(values) => values.len(),
        )*
      }
    }
  };
}

impl ValueArray {
  type_impl! {
    S8, U8,
    S16, U16,
    S32, U32,
    S64, U64,
    Ip4,
    Float,
    Double,
    Boolean,
    S8_2, S8_3, S8_4, Vs8,
    U8_2, U8_3, U8_4, Vu8,
    Boolean2, Boolean3, Boolean4, Vb,
    S16_2, S16_3, S16_4, Vs16,
    S32_2, S32_3, S32_4,
    S64_2, S64_3, S64_4,
    U16_2, U16_3, U16_4, Vu16,
    U32_2, U32_3, U32_4,
    U64_2, U64_3, U64_4,
    Float2, Float3, Float4,
    Double2, Double3, Double4,
  }
}

fn write_values<T: fmt::Display>(f: &mut fmt::Formatter, values: &[T]) -> fmt::Result {
  for (i, v) in values.iter().enumerate() {
    if i > 0 {
      f.write_str(" ")?;
    }
    fmt::Display::fmt(v, f)?;
  }
  Ok(())
}

macro_rules! write_array {
  ($method:ident, $num:expr) => {
    fn $method<T: fmt::Display>(f: &mut fmt::Formatter, values: &[[T; $num]]) -> fmt::Result {
      for (i, v) in values.iter().flat_map(|v| v.into_iter()).enumerate() {
        if i > 0 {
          f.write_str(" ")?;
        }
        fmt::Display::fmt(v, f)?;
      }
      Ok(())
    }
  };
}

write_array!(write_array_2, 2);
write_array!(write_array_3, 3);
write_array!(write_array_4, 4);
write_array!(write_array_8, 8);
write_array!(write_array_16, 16);

impl fmt::Display for ValueArray {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      ValueArray::S8(v) => write_values(f, v),
      ValueArray::U8(v) => write_values(f, v),
      ValueArray::S16(v) => write_values(f, v),
      ValueArray::U16(v) => write_values(f, v),
      ValueArray::S32(v) => write_values(f, v),
      ValueArray::U32(v) => write_values(f, v),
      ValueArray::S64(v) => write_values(f, v),
      ValueArray::U64(v) => write_values(f, v),
      ValueArray::Ip4(v) => write_values(f, v),
      ValueArray::Float(v) => write_values(f, v),
      ValueArray::Double(v) => write_values(f, v),
      ValueArray::S8_2(v) => write_array_2(f, v),
      ValueArray::U8_2(v) => write_array_2(f, v),
      ValueArray::S16_2(v) => write_array_2(f, v),
      ValueArray::U16_2(v) => write_array_2(f, v),
      ValueArray::S32_2(v) => write_array_2(f, v),
      ValueArray::U32_2(v) => write_array_2(f, v),
      ValueArray::S64_2(v) => write_array_2(f, v),
      ValueArray::U64_2(v) => write_array_2(f, v),
      ValueArray::Float2(v) => write_array_2(f, v),
      ValueArray::Double2(v) => write_array_2(f, v),
      ValueArray::S8_3(v) => write_array_3(f, v),
      ValueArray::U8_3(v) => write_array_3(f, v),
      ValueArray::S16_3(v) => write_array_3(f, v),
      ValueArray::U16_3(v) => write_array_3(f, v),
      ValueArray::S32_3(v) => write_array_3(f, v),
      ValueArray::U32_3(v) => write_array_3(f, v),
      ValueArray::S64_3(v) => write_array_3(f, v),
      ValueArray::U64_3(v) => write_array_3(f, v),
      ValueArray::Float3(v) => write_array_3(f, v),
      ValueArray::Double3(v) => write_array_3(f, v),
      ValueArray::S8_4(v) => write_array_4(f, v),
      ValueArray::U8_4(v) => write_array_4(f, v),
      ValueArray::S16_4(v) => write_array_4(f, v),
      ValueArray::U16_4(v) => write_array_4(f, v),
      ValueArray::S32_4(v) => write_array_4(f, v),
      ValueArray::U32_4(v) => write_array_4(f, v),
      ValueArray::S64_4(v) => write_array_4(f, v),
      ValueArray::U64_4(v) => write_array_4(f, v),
      ValueArray::Float4(v) => write_array_4(f, v),
      ValueArray::Double4(v) => write_array_4(f, v),
      ValueArray::Vs8(v) => write_array_16(f, v),
      ValueArray::Vu8(v) => write_array_16(f, v),
      ValueArray::Vs16(v) => write_array_8(f, v),
      ValueArray::Vu16(v) => write_array_8(f, v),
      ValueArray::Boolean(v) => write_values(f, &v),
      ValueArray::Boolean2(v) => write_array_2(f, v),
      ValueArray::Boolean3(v) => write_array_3(f, v),
      ValueArray::Boolean4(v) => write_array_4(f, v),
      ValueArray::Vb(v) => write_array_16(f, v),
    }
  }
}
