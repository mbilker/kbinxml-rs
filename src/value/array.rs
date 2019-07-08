//use std::convert::TryFrom;
use std::fmt;
use std::net::Ipv4Addr;
//use std::str::FromStr;

use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use failure::ResultExt;
//use rustc_hex::FromHex;

use crate::error::{KbinError, KbinErrorKind};
use crate::node_types::{self, StandardType};

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

macro_rules! tuple {
  (
    byte: [
      s8: [$($s8_konst:ident),*],
      u8: [$($u8_konst:ident),*],
      bool: [$($bool_konst:ident),*]
    ],
    multi: [
      $($read_method:ident $write_method:ident $inner_type:ty => [$($multi_konst:ident),*]),*
    ]
  ) => {
    pub fn from_standard_type(node_type: StandardType, input: &[u8]) -> Result<Option<ValueArray>, KbinError> {
      let node_size = node_type.size * node_type.count;
      let len = input.len() / node_size;

      // Prevent reading incomplete input data
      if node_size * len != input.len() {
        return Err(KbinErrorKind::SizeMismatch(*node_type, node_size, input.len()).into());
      }

      let value = match node_type {
        StandardType::NodeStart |
        StandardType::NodeEnd |
        StandardType::FileEnd |
        StandardType::Attribute |
        StandardType::Binary |
        StandardType::String |
        StandardType::Time => return Ok(None),
        StandardType::S8 => ValueArray::S8(input.iter().map(|v| *v as i8).collect()),
        StandardType::U8 => ValueArray::U8(input.to_vec()),
        StandardType::S16 => {
          let mut values = vec![0; len];
          BigEndian::read_i16_into(input, &mut values);
          ValueArray::S16(values)
        },
        StandardType::U16 => {
          let mut values = vec![0; len];
          BigEndian::read_u16_into(input, &mut values);
          ValueArray::U16(values)
        },
        StandardType::S32 => {
          let mut values = vec![0; len];
          BigEndian::read_i32_into(input, &mut values);
          ValueArray::S32(values)
        },
        StandardType::U32 => {
          let mut values = vec![0; len];
          BigEndian::read_u32_into(input, &mut values);
          ValueArray::U32(values)
        },
        StandardType::S64 => {
          let mut values = vec![0; len];
          BigEndian::read_i64_into(input, &mut values);
          ValueArray::S64(values)
        },
        StandardType::U64 => {
          let mut values = vec![0; len];
          BigEndian::read_u64_into(input, &mut values);
          ValueArray::U64(values)
        },
        StandardType::Ip4 => {
          let mut values = Vec::with_capacity(len);

          for chunk in input.chunks(4) {
            let mut octets = [0; 4];
            octets.copy_from_slice(chunk);
            values.push(Ipv4Addr::from(octets));
          }

          ValueArray::Ip4(values)
        },
        StandardType::Float => {
          let mut values = vec![0.0; len];
          BigEndian::read_f32_into(input, &mut values);
          ValueArray::Float(values)
        },
        StandardType::Double => {
          let mut values = vec![0.0; len];
          BigEndian::read_f64_into(input, &mut values);
          ValueArray::Double(values)
        },
        StandardType::Boolean => {
          let mut values = Vec::with_capacity(len);

          for value in input {
            match value {
              0x00 => values.push(false),
              0x01 => values.push(true),
              input => return Err(KbinErrorKind::InvalidBooleanInput(*input).into()),
            };
          }

          ValueArray::Boolean(values)
        },
        $(
          StandardType::$s8_konst => {
            const COUNT: usize = node_types::$s8_konst.count;

            let mut values = Vec::with_capacity(len);

            for chunk in input.chunks(COUNT) {
              let mut value = [0; COUNT];
              for i in 0..COUNT {
                value[i] = chunk[i] as i8;
              }
              values.push(value);
            }

            ValueArray::$s8_konst(values)
          },
        )*
        $(
          StandardType::$u8_konst => {
            const COUNT: usize = node_types::$u8_konst.count;

            let mut values = Vec::with_capacity(len);

            for chunk in input.chunks(COUNT) {
              let mut value = [0; COUNT];
              value[0..COUNT].copy_from_slice(&chunk[0..COUNT]);
              values.push(value);
            }

            ValueArray::$u8_konst(values)
          },
        )*
        $(
          StandardType::$bool_konst => {
            const COUNT: usize = node_types::$bool_konst.count;

            let mut values = Vec::with_capacity(len);

            for chunk in input.chunks(COUNT) {
              let mut value: [_; COUNT] = Default::default();
              for i in 0..COUNT {
                value[i] = match chunk[i] {
                  0x00 => false,
                  0x01 => true,
                  input => return Err(KbinErrorKind::InvalidBooleanInput(input).into()),
                };
              }
              values.push(value);
            }

            ValueArray::$bool_konst(values)
          },
        )*
        $(
          $(
            StandardType::$multi_konst => {
              const COUNT: usize = node_types::$multi_konst.count;
              const SIZE: usize = node_types::$multi_konst.size * COUNT;

              let mut values = Vec::with_capacity(len);

              for chunk in input.chunks(SIZE) {
                let mut value: [_; COUNT] = Default::default();
                BigEndian::$read_method(chunk, &mut value);
                values.push(value);
              }

              ValueArray::$multi_konst(values)
            },
          )*
        )*
      };

      Ok(Some(value))
    }

    pub(super) fn to_bytes_inner(&self, output: &mut Vec<u8>) -> Result<(), KbinError> {
      macro_rules! gen_error {
        ($konst:ident) => {
          KbinErrorKind::DataWrite(StandardType::$konst.name)
        };
      }

      let node_size = self.standard_type().size;

      match self {
        ValueArray::S8(ref values) => {
          output.reserve(values.len());
          for n in values {
            output.push(*n as u8);
          }
        },
        ValueArray::U8(ref values) => output.extend(values),
        ValueArray::S16(ref values) => {
          output.reserve(values.len() * node_size);
          for n in values {
            output.write_i16::<BigEndian>(*n).context(gen_error!(S16))?;
          }
        },
        ValueArray::U16(ref values) => {
          output.reserve(values.len() * node_size);
          for n in values {
            output.write_u16::<BigEndian>(*n).context(gen_error!(U16))?;
          }
        },
        ValueArray::S32(ref values) => {
          output.reserve(values.len() * node_size);
          for n in values {
            output.write_i32::<BigEndian>(*n).context(gen_error!(S32))?;
          }
        },
        ValueArray::U32(ref values) => {
          output.reserve(values.len() * node_size);
          for n in values {
            output.write_u32::<BigEndian>(*n).context(gen_error!(U32))?;
          }
        },
        ValueArray::S64(ref values) => {
          output.reserve(values.len() * node_size);
          for n in values {
            output.write_i64::<BigEndian>(*n).context(gen_error!(S64))?;
          }
        },
        ValueArray::U64(ref values) => {
          output.reserve(values.len() * node_size);
          for n in values {
            output.write_u64::<BigEndian>(*n).context(gen_error!(U64))?;
          }
        },
        ValueArray::Ip4(ref values) => {
          for addr in values {
            output.extend_from_slice(&addr.octets());
          }
        },
        ValueArray::Float(ref values) => {
          output.reserve(values.len() * node_size);
          for n in values {
            output.write_f32::<BigEndian>(*n).context(gen_error!(Float))?;
          }
        },
        ValueArray::Double(ref values) => {
          output.reserve(values.len() * node_size);
          for n in values {
            output.write_f64::<BigEndian>(*n).context(gen_error!(Double))?;
          }
        },
        ValueArray::Boolean(ref values) => {
          output.reserve(values.len() * node_size);
          for v in values {
            output.push(if *v { 0x01 } else { 0x00 });
          }
        },
        $(
          ValueArray::$s8_konst(values) => {
            output.reserve(values.len() * node_size);
            for value in values {
              for n in value.into_iter() {
                output.push(*n as u8);
              }
            }
          },
        )*
        $(
          ValueArray::$u8_konst(values) => {
            output.reserve(values.len() * node_size);
            for value in values {
              for n in value.into_iter() {
                output.push(*n);
              }
            }
          },
        )*
        $(
          ValueArray::$bool_konst(values) => {
            output.reserve(values.len() * node_size);
            for value in values {
              for v in value.into_iter() {
                output.push(if *v { 0x01 } else { 0x00 });
              }
            }
          },
        )*
        $(
          $(
            ValueArray::$multi_konst(values) => {
              output.reserve(values.len() * node_size);
              for value in values {
                for v in value.into_iter() {
                  output.$write_method::<BigEndian>(*v).context(gen_error!($multi_konst))?;
                }
              }
            },
          )*
        )*
      };

      Ok(())
    }
  };
}

impl ValueArray {
  tuple! {
    byte: [
      s8: [S8_2, S8_3, S8_4, Vs8],
      u8: [U8_2, U8_3, U8_4, Vu8],
      bool: [Boolean2, Boolean3, Boolean4, Vb]
    ],
    multi: [
      read_i16_into write_i16 i16 => [S16_2, S16_3, S16_4, Vs16],
      read_i32_into write_i32 i32 => [S32_2, S32_3, S32_4],
      read_i64_into write_i64 i64 => [S64_2, S64_3, S64_4],
      read_u16_into write_u16 u16 => [U16_2, U16_3, U16_4, Vu16],
      read_u32_into write_u32 u32 => [U32_2, U32_3, U32_4],
      read_u64_into write_u64 u64 => [U64_2, U64_3, U64_4],
      read_f32_into write_f32 f32 => [Float2, Float3, Float4],
      read_f64_into write_f64 f64 => [Double2, Double3, Double4]
    ]
  }

  pub fn standard_type(&self) -> StandardType {
    match self {
      ValueArray::S8(_) => StandardType::S8,
      ValueArray::U8(_) => StandardType::U8,
      ValueArray::S16(_) => StandardType::S16,
      ValueArray::U16(_) => StandardType::U16,
      ValueArray::S32(_) => StandardType::S32,
      ValueArray::U32(_) => StandardType::U32,
      ValueArray::S64(_) => StandardType::S64,
      ValueArray::U64(_) => StandardType::U64,
      ValueArray::Ip4(_) => StandardType::Ip4,
      ValueArray::Float(_) => StandardType::Float,
      ValueArray::Double(_) => StandardType::Double,
      ValueArray::S8_2(_) => StandardType::S8_2,
      ValueArray::U8_2(_) => StandardType::U8_2,
      ValueArray::S16_2(_) => StandardType::S16_2,
      ValueArray::U16_2(_) => StandardType::U16_2,
      ValueArray::S32_2(_) => StandardType::S32_2,
      ValueArray::U32_2(_) => StandardType::U32_2,
      ValueArray::S64_2(_) => StandardType::S64_2,
      ValueArray::U64_2(_) => StandardType::U64_2,
      ValueArray::Float2(_) => StandardType::Float2,
      ValueArray::Double2(_) => StandardType::Double2,
      ValueArray::S8_3(_) => StandardType::S8_3,
      ValueArray::U8_3(_) => StandardType::U8_3,
      ValueArray::S16_3(_) => StandardType::S16_3,
      ValueArray::U16_3(_) => StandardType::U16_3,
      ValueArray::S32_3(_) => StandardType::S32_3,
      ValueArray::U32_3(_) => StandardType::U32_3,
      ValueArray::S64_3(_) => StandardType::S64_3,
      ValueArray::U64_3(_) => StandardType::U64_3,
      ValueArray::Float3(_) => StandardType::Float3,
      ValueArray::Double3(_) => StandardType::Double3,
      ValueArray::S8_4(_) => StandardType::S8_4,
      ValueArray::U8_4(_) => StandardType::U8_4,
      ValueArray::S16_4(_) => StandardType::S16_4,
      ValueArray::U16_4(_) => StandardType::U16_4,
      ValueArray::S32_4(_) => StandardType::S32_4,
      ValueArray::U32_4(_) => StandardType::U32_4,
      ValueArray::S64_4(_) => StandardType::S64_4,
      ValueArray::U64_4(_) => StandardType::U64_4,
      ValueArray::Float4(_) => StandardType::Float4,
      ValueArray::Double4(_) => StandardType::Double4,
      ValueArray::Vs8(_) => StandardType::Vs8,
      ValueArray::Vu8(_) => StandardType::Vu8,
      ValueArray::Vs16(_) => StandardType::Vs16,
      ValueArray::Vu16(_) => StandardType::Vu16,
      ValueArray::Boolean(_) => StandardType::Boolean,
      ValueArray::Boolean2(_) => StandardType::Boolean2,
      ValueArray::Boolean3(_) => StandardType::Boolean3,
      ValueArray::Boolean4(_) => StandardType::Boolean4,
      ValueArray::Vb(_) => StandardType::Vb,
    }
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
