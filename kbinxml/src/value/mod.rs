use std::borrow::Cow;
use std::convert::TryFrom;
use std::fmt;
use std::io::Cursor;
use std::net::Ipv4Addr;

use rustc_hex::FromHex;
use snafu::ResultExt;

use crate::error::*;
use crate::node_types::StandardType;
use crate::types::{FromKbinBytes, FromKbinString, IntoKbinBytes};

mod array;

pub use self::array::ValueArray;

macro_rules! construct_types {
  (
    $(
      ($konst:ident, $($value_type:tt)*);
    )+
  ) => {
    #[derive(Clone, PartialEq)]
    pub enum Value {
      $(
        $konst($($value_type)*),
      )+
      Binary(Vec<u8>),
      Time(u32),
      Attribute(String),

      Array(ValueArray),
    }

    $(
      impl From<$($value_type)*> for Value {
        fn from(value: $($value_type)*) -> Value {
          Value::$konst(value)
        }
      }

      impl TryFrom<Value> for $($value_type)* {
        type Error = KbinError;

        fn try_from(value: Value) -> Result<Self> {
          match value {
            Value::$konst(v) => Ok(v),
            value => {
              Err(KbinError::ValueTypeMismatch {
                node_type: StandardType::$konst,
                value,
              })
            },
          }
        }
      }

      impl TryFrom<&Value> for $($value_type)* {
        type Error = KbinError;

        fn try_from(value: &Value) -> Result<Self> {
          match value {
            Value::$konst(ref v) => Ok(v.clone()),
            value => {
              Err(KbinError::ValueTypeMismatch {
                node_type: StandardType::$konst,
                value: value.clone(),
              })
            },
          }
        }
      }
    )+

    impl Value {
      pub fn standard_type(&self) -> StandardType {
        match *self {
          $(
            Value::$konst(_) => StandardType::$konst,
          )+
          Value::Binary(_) => StandardType::Binary,
          Value::Time(_) => StandardType::Time,
          Value::Attribute(_) => StandardType::Attribute,
          Value::Array(ref value) => value.standard_type(),
        }
      }
    }
  }
}

macro_rules! tuple {
  (
    $($konst:ident),*$(,)?
  ) => {
    pub fn from_standard_type(node_type: StandardType, is_array: bool, input: &[u8]) -> Result<Option<Value>> {
      let node_size = node_type.size * node_type.count;

      if is_array {
        let value = match ValueArray::from_standard_type(node_type, input)? {
          Some(value) => value,
          None => return Err(KbinError::InvalidState),
        };
        debug!("Value::from_standard_type({:?}) input: 0x{:02x?} => {:?}", node_type, input, value);

        return Ok(Some(Value::Array(value)));
      }

      match node_type {
        StandardType::String |
        StandardType::Binary => {},
        _ => {
          if input.len() != node_size {
            return Err(KbinError::SizeMismatch { node_type: node_type.name, expected: node_size, actual: input.len() });
          }
        },
      };

      let mut reader = Cursor::new(input);

      let value = match node_type {
        StandardType::NodeStart |
        StandardType::NodeEnd |
        StandardType::FileEnd |
        StandardType::Attribute |
        StandardType::String => return Ok(None),
        StandardType::S8 => i8::from_kbin_bytes(&mut reader).map(Value::S8)?,
        StandardType::U8 => u8::from_kbin_bytes(&mut reader).map(Value::U8)?,
        StandardType::S16 => i16::from_kbin_bytes(&mut reader).map(Value::S16)?,
        StandardType::U16 => u16::from_kbin_bytes(&mut reader).map(Value::U16)?,
        StandardType::S32 => i32::from_kbin_bytes(&mut reader).map(Value::S32)?,
        StandardType::U32 => u32::from_kbin_bytes(&mut reader).map(Value::U32)?,
        StandardType::S64 => i64::from_kbin_bytes(&mut reader).map(Value::S64)?,
        StandardType::U64 => u64::from_kbin_bytes(&mut reader).map(Value::U64)?,
        StandardType::Binary => Value::Binary(input.to_vec()),
        StandardType::Time => u32::from_kbin_bytes(&mut reader).map(Value::Time)?,
        StandardType::Ip4 => Ipv4Addr::from_kbin_bytes(&mut reader).map(Value::Ip4)?,
        StandardType::Float => f32::from_kbin_bytes(&mut reader).map(Value::Float)?,
        StandardType::Double => f64::from_kbin_bytes(&mut reader).map(Value::Double)?,
        StandardType::Boolean => bool::from_kbin_bytes(&mut reader).map(Value::Boolean)?,
        $(
          StandardType::$konst => {
            FromKbinBytes::from_kbin_bytes(&mut reader).map(Value::$konst)?
          },
        )*
      };
      debug!("Value::from_standard_type({:?}) input: 0x{:02x?} => {:?}", node_type, input, value);

      Ok(Some(value))
    }

    pub fn from_string(node_type: StandardType, input: &str, is_array: bool, arr_count: usize) -> Result<Value> {
      trace!("Value::from_string({:?}, is_array: {}, arr_count: {}) => input: {:?}", node_type, is_array, arr_count, input);

      if is_array {
        let value = match node_type.count {
          0 => return Err(KbinError::InvalidState.into()),
          count => Value::Array(ValueArray::from_string(node_type, count, input, arr_count)?),
        };
        debug!("Value::from_string({:?}) input: {:?} => {:?}", node_type, input, value);

        return Ok(value);
      }

      let value = match node_type {
        StandardType::NodeStart |
        StandardType::NodeEnd |
        StandardType::FileEnd => return Err(KbinError::InvalidNodeType { node_type }),
        StandardType::S8 => i8::from_kbin_string(input).map(Value::S8)?,
        StandardType::U8 => u8::from_kbin_string(input).map(Value::U8)?,
        StandardType::S16 => i16::from_kbin_string(input).map(Value::S16)?,
        StandardType::U16 => u16::from_kbin_string(input).map(Value::U16)?,
        StandardType::S32 => i32::from_kbin_string(input).map(Value::S32)?,
        StandardType::U32 => u32::from_kbin_string(input).map(Value::U32)?,
        StandardType::S64 => i64::from_kbin_string(input).map(Value::S64)?,
        StandardType::U64 => u64::from_kbin_string(input).map(Value::U64)?,
        StandardType::Binary => {
          let data: Vec<u8> = input.from_hex().context(HexError)?;
          Value::Binary(data)
        },
        StandardType::String => Value::String(input.to_owned()),
        StandardType::Attribute => Value::Attribute(input.to_owned()),
        StandardType::Ip4 => Ipv4Addr::from_kbin_string(input).map(Value::Ip4)?,
        StandardType::Time => u32::from_kbin_string(input).map(Value::Time)?,
        StandardType::Float => f32::from_kbin_string(input).map(Value::Float)?,
        StandardType::Double => f64::from_kbin_string(input).map(Value::Double)?,
        StandardType::Boolean => bool::from_kbin_string(input).map(Value::Boolean)?,
        $(
          StandardType::$konst => FromKbinString::from_kbin_string(input).map(Value::$konst)?,
        )*
      };
      debug!("Value::from_string({:?}) input: {:?} => {:?}", node_type, input, value);

      Ok(value)
    }

    fn to_bytes_inner(&self, output: &mut Vec<u8>) -> Result<()> {
      debug!("Value::to_bytes_inner(self: {:?})", self);

      match self {
        Value::S8(n) => n.write_kbin_bytes(output),
        Value::U8(n) => n.write_kbin_bytes(output),
        Value::S16(n) => n.write_kbin_bytes(output),
        Value::U16(n) => n.write_kbin_bytes(output),
        Value::S32(n) => n.write_kbin_bytes(output),
        Value::U32(n) => n.write_kbin_bytes(output),
        Value::S64(n) => n.write_kbin_bytes(output),
        Value::U64(n) => n.write_kbin_bytes(output),
        Value::Binary(data) => output.extend_from_slice(data),
        Value::Time(n) => n.write_kbin_bytes(output),
        Value::Ip4(addr) => addr.write_kbin_bytes(output),
        Value::Float(n) => n.write_kbin_bytes(output),
        Value::Double(n) => n.write_kbin_bytes(output),
        Value::Boolean(v) => v.write_kbin_bytes(output),
        Value::Array(value) => value.to_bytes_into(output)?,
        Value::Attribute(_) |
        Value::String(_) => return Err(KbinError::InvalidNodeType { node_type: self.standard_type() }),
        $(
          Value::$konst(value) => {
            output.reserve(value.len() * StandardType::$konst.size);
            value.write_kbin_bytes(output);
          },
        )*
      };

      Ok(())
    }
  };
}

impl Value {
    tuple! {
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

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut output = Vec::new();
        self.to_bytes_inner(&mut output)?;

        Ok(output)
    }

    #[inline]
    pub fn to_bytes_into(&self, output: &mut Vec<u8>) -> Result<()> {
        self.to_bytes_inner(output)
    }

    pub fn as_i8(&self) -> Result<i8> {
        match self {
            Value::S8(ref n) => Ok(*n),
            value => Err(KbinError::ValueTypeMismatch {
                node_type: StandardType::S8,
                value: value.clone(),
            }),
        }
    }

    pub fn as_u8(&self) -> Result<u8> {
        match self {
            Value::U8(ref n) => Ok(*n),
            value => Err(KbinError::ValueTypeMismatch {
                node_type: StandardType::U8,
                value: value.clone(),
            }),
        }
    }

    pub fn as_i16(&self) -> Result<i16> {
        match self {
            Value::S16(ref n) => Ok(*n),
            value => Err(KbinError::ValueTypeMismatch {
                node_type: StandardType::S16,
                value: value.clone(),
            }),
        }
    }

    pub fn as_u16(&self) -> Result<u16> {
        match self {
            Value::U16(ref n) => Ok(*n),
            value => Err(KbinError::ValueTypeMismatch {
                node_type: StandardType::U16,
                value: value.clone(),
            }),
        }
    }

    pub fn as_i32(&self) -> Result<i32> {
        match self {
            Value::S32(ref n) => Ok(*n),
            value => Err(KbinError::ValueTypeMismatch {
                node_type: StandardType::S32,
                value: value.clone(),
            }),
        }
    }

    pub fn as_u32(&self) -> Result<u32> {
        match self {
            Value::U32(ref n) => Ok(*n),
            value => Err(KbinError::ValueTypeMismatch {
                node_type: StandardType::U32,
                value: value.clone(),
            }),
        }
    }

    pub fn as_i64(&self) -> Result<i64> {
        match self {
            Value::S64(ref n) => Ok(*n),
            value => Err(KbinError::ValueTypeMismatch {
                node_type: StandardType::S64,
                value: value.clone(),
            }),
        }
    }

    pub fn as_u64(&self) -> Result<u64> {
        match self {
            Value::U64(ref n) => Ok(*n),
            value => Err(KbinError::ValueTypeMismatch {
                node_type: StandardType::U64,
                value: value.clone(),
            }),
        }
    }

    pub fn as_slice(&self) -> Result<&[u8]> {
        match self {
            Value::Binary(ref data) => Ok(data),
            value => Err(KbinError::ValueTypeMismatch {
                node_type: StandardType::Binary,
                value: value.clone(),
            }),
        }
    }

    pub fn as_str(&self) -> Result<&str> {
        match self {
            Value::String(ref s) => Ok(s),
            value => Err(KbinError::ValueTypeMismatch {
                node_type: StandardType::String,
                value: value.clone(),
            }),
        }
    }

    pub fn as_string(self) -> Result<String> {
        match self {
            Value::String(s) => Ok(s),
            value => Err(KbinError::ValueTypeMismatch {
                node_type: StandardType::String,
                value,
            }),
        }
    }

    pub fn as_attribute(self) -> Result<String> {
        match self {
            Value::Attribute(s) => Ok(s),
            value => Err(KbinError::ValueTypeMismatch {
                node_type: StandardType::Attribute,
                value,
            }),
        }
    }

    pub fn as_binary(&self) -> Result<&[u8]> {
        match self {
            Value::Binary(ref data) => Ok(data),
            value => Err(KbinError::ValueTypeMismatch {
                node_type: StandardType::Binary,
                value: value.clone(),
            }),
        }
    }

    pub fn as_array(&self) -> Result<&ValueArray> {
        match self {
            Value::Array(ref values) => Ok(values),
            value => Err(KbinError::ExpectedValueArray {
                value: value.clone(),
            }),
        }
    }

    pub fn into_binary(self) -> Result<Vec<u8>> {
        match self {
            Value::Binary(data) => Ok(data),
            value => Err(KbinError::ValueTypeMismatch {
                node_type: StandardType::Binary,
                value,
            }),
        }
    }
}

/*
impl TryFrom<Value> for Vec<Value> {
  type Error = KbinError;

  fn try_from(value: Value) -> Result<Self, Self::Error> {
    match value {
      Value::Array(values) => Ok(values),
      value => Err(KbinError::ExpectedValueArray(value).into()),
    }
  }
}
*/

impl TryFrom<Value> for Vec<u8> {
    type Error = KbinError;

    fn try_from(value: Value) -> Result<Self> {
        // An array of unsigned 8-bit integers can either be `Binary` or a literal
        // array of unsigned 8-bit integers.
        match value {
            Value::Binary(data) => Ok(data),
            Value::Array(values) => match values {
                ValueArray::U8(values) => Ok(values),
                values => Err(KbinError::ValueTypeMismatch {
                    node_type: StandardType::U8,
                    value: Value::Array(values),
                }),
            },
            value => Err(KbinError::ValueTypeMismatch {
                node_type: StandardType::Binary,
                value,
            }),
        }
    }
}

impl TryFrom<&Value> for Vec<u8> {
    type Error = KbinError;

    fn try_from(value: &Value) -> Result<Self> {
        match value {
            Value::Binary(ref data) => Ok(data.to_vec()),
            Value::Array(ref values) => match values.clone() {
                ValueArray::U8(values) => Ok(values),
                values => Err(KbinError::ValueTypeMismatch {
                    node_type: StandardType::U8,
                    value: Value::Array(values),
                }),
            },
            value => Err(KbinError::ValueTypeMismatch {
                node_type: StandardType::Binary,
                value: value.clone(),
            }),
        }
    }
}

impl TryFrom<Value> for Cow<'_, str> {
    type Error = KbinError;

    fn try_from(value: Value) -> Result<Self> {
        match value {
            Value::String(v) => Ok(Cow::Owned(v)),
            value => Err(KbinError::ValueTypeMismatch {
                node_type: StandardType::String,
                value,
            }),
        }
    }
}

impl TryFrom<&Value> for Cow<'_, str> {
    type Error = KbinError;

    fn try_from(value: &Value) -> Result<Self> {
        match value {
            Value::String(ref v) => Ok(Cow::Owned(v.clone())),
            value => Err(KbinError::ValueTypeMismatch {
                node_type: StandardType::String,
                value: value.clone(),
            }),
        }
    }
}

impl From<Vec<u8>> for Value {
    fn from(value: Vec<u8>) -> Value {
        Value::Binary(value)
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        macro_rules! field {
      (
        display: [$($konst_display:ident),*],
        debug: [$($konst_debug:ident),*]
      ) => {
        match *self {
          $(
            Value::$konst_display(ref v) => write!(f, concat!(stringify!($konst_display), "({})"), v),
          )*
          $(
            Value::$konst_debug(ref v) => write!(f, concat!(stringify!($konst_debug), "({:?})"), v),
          )*
          Value::Binary(ref v) => write!(f, "Binary(0x{:02x?})", v),
          Value::Array(ref value) => if f.alternate() {
            write!(f, "Array({:#?})", value)
          } else {
            write!(f, "Array({:?})", value)
          },
        }
      };
    }

        field! {
          display: [
            S8, S16, S32, S64,
            U8, U16, U32, U64,
            Float, Double, Boolean
          ],
          debug: [
            String, Time, Ip4,
            Attribute,
            S8_2, U8_2, S16_2, U16_2, S32_2, U32_2, S64_2, U64_2, Float2, Double2, Boolean2,
            S8_3, U8_3, S16_3, U16_3, S32_3, U32_3, S64_3, U64_3, Float3, Double3, Boolean3,
            S8_4, U8_4, S16_4, U16_4, S32_4, U32_4, S64_4, U64_4, Float4, Double4, Boolean4,
            Vs16, Vu16,
            Vs8, Vu8, Vb
          ]
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        macro_rules! display_value {
      (
        simple: [$($simple:ident),*],
        tuple: [$($tuple:ident),*],
        value: [$($parent:ident => [$($child:ident),*]),*]
      ) => {
        match self {
          $(
            Value::$simple(v) => fmt::Display::fmt(v, f),
          )*
          $(
            Value::$tuple(values) => {
              for (i, v) in values.iter().enumerate() {
                if i > 0 {
                  f.write_str(" ")?;
                }
                fmt::Display::fmt(v, f)?;
              }
              Ok(())
            },
          )*
          $(
            $(
              Value::$child(values) => {
                for (i, v) in values.iter().enumerate() {
                  if i > 0 {
                    f.write_str(" ")?;
                  }
                  fmt::Display::fmt(&Value::$parent(*v), f)?;
                }
                Ok(())
              },
            )*
          )*
          Value::Binary(buf) => {
            for n in buf {
              write!(f, "{:02x}", n)?;
            }
            Ok(())
          },
          Value::Float(n) => write!(f, "{:.6}", n),
          Value::Double(n) => write!(f, "{:.6}", n),
          Value::Boolean(b) => match b {
            true => f.write_str("1"),
            false => f.write_str("0"),
          },
        }
      };
    }

        display_value! {
          simple: [
            S8, U8, S16, U16, S32, U32, S64, U64,
            String, Ip4, Time, Attribute,
            Array
          ],
          tuple: [
            S8_2, U8_2, S16_2, U16_2, S32_2, U32_2, S64_2, U64_2,
            S8_3, U8_3, S16_3, U16_3, S32_3, U32_3, S64_3, U64_3,
            S8_4, U8_4, S16_4, U16_4, S32_4, U32_4, S64_4, U64_4,
            Vs8, Vu8, Vs16, Vu16
          ],
          value: [
            Float => [Float2, Float3, Float4],
            Double => [Double2, Double3, Double4],
            Boolean => [Boolean2, Boolean3, Boolean4, Vb]
          ]
        }
    }
}

construct_types! {
  (S8,       i8);
  (U8,       u8);
  (S16,      i16);
  (U16,      u16);
  (S32,      i32);
  (U32,      u32);
  (S64,      i64);
  (U64,      u64);
  //(Binary,   Vec<u8>);
  (String,   String);
  (Ip4,      Ipv4Addr);
  //(Time,     u32);
  (Float,    f32);
  (Double,   f64);
  (S8_2,     [i8; 2]);
  (U8_2,     [u8; 2]);
  (S16_2,    [i16; 2]);
  (U16_2,    [u16; 2]);
  (S32_2,    [i32; 2]);
  (U32_2,    [u32; 2]);
  (S64_2,    [i64; 2]);
  (U64_2,    [u64; 2]);
  (Float2,   [f32; 2]);
  (Double2,  [f64; 2]);
  (S8_3,     [i8; 3]);
  (U8_3,     [u8; 3]);
  (S16_3,    [i16; 3]);
  (U16_3,    [u16; 3]);
  (S32_3,    [i32; 3]);
  (U32_3,    [u32; 3]);
  (S64_3,    [i64; 3]);
  (U64_3,    [u64; 3]);
  (Float3,   [f32; 3]);
  (Double3,  [f64; 3]);
  (S8_4,     [i8; 4]);
  (U8_4,     [u8; 4]);
  (S16_4,    [i16; 4]);
  (U16_4,    [u16; 4]);
  (S32_4,    [i32; 4]);
  (U32_4,    [u32; 4]);
  (S64_4,    [i64; 4]);
  (U64_4,    [u64; 4]);
  (Float4,   [f32; 4]);
  (Double4,  [f64; 4]);
  //(Attribute, String);
  // no 47
  (Vs8,      [i8; 16]);
  (Vu8,      [u8; 16]);
  (Vs16,     [i16; 8]);
  (Vu16,     [u16; 8]);
  (Boolean,  bool);
  (Boolean2, [bool; 2]);
  (Boolean3, [bool; 3]);
  (Boolean4, [bool; 4]);
  (Vb,       [bool; 16]);
}
