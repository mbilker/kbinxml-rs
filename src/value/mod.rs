use std::convert::TryFrom;
use std::fmt;
use std::io::Cursor;
use std::net::Ipv4Addr;
use std::str::FromStr;

use failure::{Fail, ResultExt};
use rustc_hex::FromHex;

use crate::error::{KbinError, KbinErrorKind};
use crate::node_types::{self, StandardType};
use crate::types::{FromKbinBytes, IntoKbinBytes};

mod array;

pub use self::array::ValueArray;

#[inline]
fn parse<T>(node_type: StandardType, input: &str) -> Result<T, KbinError>
  where T: FromStr,
        T::Err: Fail
{
  // Check for space character
  if input.find(' ').is_some() {
    return Err(KbinErrorKind::InvalidState.into());
  }

  let n = input.parse::<T>().context(KbinErrorKind::StringParse(node_type.name))?;
  Ok(n)
}

#[inline]
fn parse_tuple<T>(node_type: StandardType, input: &str, output: &mut [T]) -> Result<(), KbinError>
  where T: FromStr,
        T::Err: Fail
{
  let count = input.split(' ').count();
  if count != node_type.count {
    return Err(KbinErrorKind::SizeMismatch(*node_type, node_type.count, count).into());
  }

  for (i, part) in input.split(' ').enumerate() {
    output[i] = part.parse::<T>().context(KbinErrorKind::StringParse(node_type.name))?;
  }

  Ok(())
}

fn to_array(node_type: StandardType, count: usize, input: &str, arr_count: usize) -> Result<Value, KbinError> {
  let mut i = 0;
  trace!("to_array(count: {}, input: {:?}, arr_count: {})", count, input, arr_count);
  let iter = input.split(|c| {
    if c == ' ' {
      // Increment the space counter
      i += 1;

      // If the space counter is equal to count, then split
      let res = i == count;

      // If splitting, reset the counter
      if res {
        i = 0;
      }

      res
    } else {
      false
    }
  });

  let mut values = Vec::new();

  for part in iter {
    trace!("part: {:?}", part);
    values.push(Value::from_string(node_type, part, false, 1)?);
  }

  Ok(Value::Array(node_type, values))
}

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

      Array(StandardType, Vec<Value>),
      ArrayNew(ValueArray),
    }

    $(
      impl From<$($value_type)*> for Value {
        fn from(value: $($value_type)*) -> Value {
          Value::$konst(value)
        }
      }

      impl TryFrom<Value> for $($value_type)* {
        type Error = KbinError;

        fn try_from(value: Value) -> Result<Self, Self::Error> {
          match value {
            Value::$konst(v) => Ok(v),
            value => Err(KbinErrorKind::ValueTypeMismatch(StandardType::$konst, value).into()),
          }
        }
      }

      impl TryFrom<&Value> for $($value_type)* {
        type Error = KbinError;

        fn try_from(value: &Value) -> Result<Self, Self::Error> {
          match value {
            Value::$konst(ref v) => Ok(v.clone()),
            value => Err(KbinErrorKind::ValueTypeMismatch(StandardType::$konst, value.clone()).into()),
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
          Value::Array(node_type, _) => node_type,
          Value::ArrayNew(ref value) => value.standard_type(),
        }
      }
    }
  }
}

macro_rules! tuple {
  (
    byte: [
      int: [
        $($int_konst:ident),*$(,)?
      ],
      bool: [$($bool_konst:ident),*]
    ],
    multi: [
      $($inner_type:ty => [$($multi_konst:ident),*]),*
    ]
  ) => {
    pub fn from_standard_type(node_type: StandardType, is_array: bool, input: &[u8]) -> Result<Option<Value>, KbinError> {
      let node_size = node_type.size * node_type.count;

      if is_array {
        let value = match ValueArray::from_standard_type(node_type, input)? {
          Some(value) => value,
          None => return Err(KbinErrorKind::InvalidState.into()),
        };
        debug!("Value::from_standard_type({:?}) input: 0x{:02x?} => {:?}", node_type, input, value);

        return Ok(Some(Value::ArrayNew(value)));
      }

      match node_type {
        StandardType::String |
        StandardType::Binary => {},
        _ => {
          if input.len() != node_size {
            return Err(KbinErrorKind::SizeMismatch(*node_type, node_size, input.len()).into());
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
          StandardType::$int_konst => {
            FromKbinBytes::from_kbin_bytes(&mut reader).map(Value::$int_konst)?
          },
        )*
        $(
          StandardType::$bool_konst => {
            FromKbinBytes::from_kbin_bytes(&mut reader).map(Value::$bool_konst)?
          },
        )*
        $(
          $(
            StandardType::$multi_konst => {
              FromKbinBytes::from_kbin_bytes(&mut reader).map(Value::$multi_konst)?
            },
          )*
        )*
      };
      debug!("Value::from_standard_type({:?}) input: 0x{:02x?} => {:?}", node_type, input, value);

      Ok(Some(value))
    }

    pub fn from_string(node_type: StandardType, input: &str, is_array: bool, arr_count: usize) -> Result<Value, KbinError> {
      if is_array {
        let value = match node_type.count {
          1 => {
            // May have a node (i.e. `Ip4`) that is only a single count, but it
            // can be part of an array
            match arr_count {
              0 => return Err(KbinErrorKind::InvalidState.into()),
              1 => Value::from_string(node_type, input, false, arr_count)?,
              _ => to_array(node_type, node_type.count, input, arr_count)?,
            }
          },
          count if count > 1 => to_array(node_type, count, input, arr_count)?,
          _ => return Err(KbinErrorKind::InvalidState.into()),
        };
        debug!("Value::from_string({:?}) input: {:?} => {:?}", node_type, input, value);

        return Ok(value);
      }

      let value = match node_type {
        StandardType::S8 => Value::S8(parse::<i8>(node_type, input)?),
        StandardType::U8 => Value::U8(parse::<u8>(node_type, input)?),
        StandardType::S16 => Value::S16(parse::<i16>(node_type, input)?),
        StandardType::U16 => Value::U16(parse::<u16>(node_type, input)?),
        StandardType::S32 => Value::S32(parse::<i32>(node_type, input)?),
        StandardType::U32 => Value::U32(parse::<u32>(node_type, input)?),
        StandardType::S64 => Value::S64(parse::<i64>(node_type, input)?),
        StandardType::U64 => Value::U64(parse::<u64>(node_type, input)?),
        StandardType::Binary => {
          let data: Vec<u8> = input.from_hex().context(KbinErrorKind::HexError)?;
          Value::Binary(data)
        },
        StandardType::String => Value::String(input.to_owned()),
        StandardType::Attribute => Value::Attribute(input.to_owned()),
        StandardType::Ip4 => {
          let mut i = 0;
          let mut octets = [0; 4];

          // IP Addresses are split by a period, don't use `parse_tuple`
          for part in input.split('.') {
            octets[i] = parse::<u8>(node_type, part)?;
            i += 1;
          }

          if i != 4 {
            return Err(KbinErrorKind::SizeMismatch(*node_type, 4, i).into());
          }

          Value::Ip4(Ipv4Addr::from(octets))
        },
        StandardType::Time => Value::Time(parse::<u32>(node_type, input)?),
        StandardType::Float => Value::Float(parse::<f32>(node_type, input)?),
        StandardType::Double => Value::Double(parse::<f64>(node_type, input)?),
        StandardType::Boolean => Value::Boolean(match input {
          "0" => false,
          "1" => true,
          v => return Err(KbinErrorKind::InvalidBooleanInput(parse::<u8>(node_type, v)?).into()),
        }),
        StandardType::NodeEnd |
        StandardType::FileEnd |
        StandardType::NodeStart => return Err(KbinErrorKind::InvalidNodeType(node_type).into()),
        $(
          StandardType::$int_konst => {
            const COUNT: usize = node_types::$int_konst.count;
            let mut value = [0; COUNT];
            parse_tuple(node_type, input, &mut value)?;
            Value::$int_konst(value)
          },
        )*
        $(
          StandardType::$bool_konst => {
            const COUNT: usize = node_types::$bool_konst.count;
            let mut i = 0;
            let mut value: [_; COUNT] = Default::default();
            for part in input.split(' ') {
              value[i] = match part {
                "0" => false,
                "1" => true,
                v => return Err(KbinErrorKind::InvalidBooleanInput(parse::<u8>(node_type, v)?).into()),
              };
              i += 1;
            }

            if i != COUNT {
              return Err(KbinErrorKind::SizeMismatch(*node_type, COUNT, i).into());
            }

            Value::$bool_konst(value)
          },
        )*
        $(
          $(
            StandardType::$multi_konst => {
              const COUNT: usize = node_types::$multi_konst.count;
              let mut value: [_; COUNT] = Default::default();
              parse_tuple::<$inner_type>(node_type, input, &mut value)?;
              Value::$multi_konst(value)
            },
          )*
        )*
      };
      debug!("Value::from_string({:?}) input: {:?} => {:?}", node_type, input, value);

      Ok(value)
    }

    fn to_bytes_inner(&self, output: &mut Vec<u8>) -> Result<(), KbinError> {
      debug!("Value::to_bytes_inner(self: {:?})", self);

      match self {
        Value::S8(ref n) => n.write_kbin_bytes(output),
        Value::U8(ref n) => n.write_kbin_bytes(output),
        Value::S16(ref n) => n.write_kbin_bytes(output),
        Value::U16(ref n) => n.write_kbin_bytes(output),
        Value::S32(ref n) => n.write_kbin_bytes(output),
        Value::U32(ref n) => n.write_kbin_bytes(output),
        Value::S64(ref n) => n.write_kbin_bytes(output),
        Value::U64(ref n) => n.write_kbin_bytes(output),
        Value::Binary(ref data) => output.extend_from_slice(data),
        Value::Time(ref n) => n.write_kbin_bytes(output),
        Value::Ip4(addr) => addr.write_kbin_bytes(output),
        Value::Float(ref n) => n.write_kbin_bytes(output),
        Value::Double(ref n) => n.write_kbin_bytes(output),
        Value::Boolean(ref v) => v.write_kbin_bytes(output),
        Value::Array(_, values) => {
          for value in values {
            value.to_bytes_inner(output)?;
          }
        },
        Value::ArrayNew(value) => value.to_bytes_inner(output)?,
        Value::Attribute(_) |
        Value::String(_) => return Err(KbinErrorKind::InvalidNodeType(self.standard_type()).into()),
        $(
          Value::$int_konst(value) => {
            output.reserve(value.len());
            value.write_kbin_bytes(output);
          },
        )*
        $(
          Value::$bool_konst(value) => {
            output.reserve(value.len());
            value.write_kbin_bytes(output);
          },
        )*
        $(
          $(
            Value::$multi_konst(value) => {
              output.reserve(value.len() * StandardType::$multi_konst.size);
              value.write_kbin_bytes(output);
            },
          )*
        )*
      };

      Ok(())
    }
  };
}

impl Value {
  tuple! {
    byte: [
      int: [
        S8_2, S8_3, S8_4, Vs8,
        U8_2, U8_3, U8_4, Vu8,
      ],
      bool: [Boolean2, Boolean3, Boolean4, Vb]
    ],
    multi: [
      i16 => [S16_2, S16_3, S16_4, Vs16],
      i32 => [S32_2, S32_3, S32_4],
      i64 => [S64_2, S64_3, S64_4],
      u16 => [U16_2, U16_3, U16_4, Vu16],
      u32 => [U32_2, U32_3, U32_4],
      u64 => [U64_2, U64_3, U64_4],
      f32 => [Float2, Float3, Float4],
      f64 => [Double2, Double3, Double4]
    ]
  }

  pub fn to_bytes(&self) -> Result<Vec<u8>, KbinError> {
    let mut output = Vec::new();
    self.to_bytes_inner(&mut output)?;

    Ok(output)
  }

  #[inline]
  pub fn to_bytes_into(&self, output: &mut Vec<u8>) -> Result<(), KbinError> {
    self.to_bytes_inner(output)
  }

  #[inline]
  pub fn array_as_string(values: &[Value]) -> String {
    BorrowedValueArray(values).to_string()
  }

  pub fn as_i8(&self) -> Result<i8, KbinError> {
    match self {
      Value::S8(ref n) => Ok(*n),
      value => Err(KbinErrorKind::ValueTypeMismatch(StandardType::S8, value.clone()).into()),
    }
  }

  pub fn as_u8(&self) -> Result<u8, KbinError> {
    match self {
      Value::U8(ref n) => Ok(*n),
      value => Err(KbinErrorKind::ValueTypeMismatch(StandardType::U8, value.clone()).into()),
    }
  }

  pub fn as_i16(&self) -> Result<i16, KbinError> {
    match self {
      Value::S16(ref n) => Ok(*n),
      value => Err(KbinErrorKind::ValueTypeMismatch(StandardType::S16, value.clone()).into()),
    }
  }

  pub fn as_u16(&self) -> Result<u16, KbinError> {
    match self {
      Value::U16(ref n) => Ok(*n),
      value => Err(KbinErrorKind::ValueTypeMismatch(StandardType::U16, value.clone()).into()),
    }
  }

  pub fn as_i32(&self) -> Result<i32, KbinError> {
    match self {
      Value::S32(ref n) => Ok(*n),
      value => Err(KbinErrorKind::ValueTypeMismatch(StandardType::S32, value.clone()).into()),
    }
  }

  pub fn as_u32(&self) -> Result<u32, KbinError> {
    match self {
      Value::U32(ref n) => Ok(*n),
      value => Err(KbinErrorKind::ValueTypeMismatch(StandardType::U32, value.clone()).into()),
    }
  }

  pub fn as_i64(&self) -> Result<i64, KbinError> {
    match self {
      Value::S64(ref n) => Ok(*n),
      value => Err(KbinErrorKind::ValueTypeMismatch(StandardType::S64, value.clone()).into()),
    }
  }

  pub fn as_u64(&self) -> Result<u64, KbinError> {
    match self {
      Value::U64(ref n) => Ok(*n),
      value => Err(KbinErrorKind::ValueTypeMismatch(StandardType::U64, value.clone()).into()),
    }
  }

  pub fn as_slice(&self) -> Result<&[u8], KbinError> {
    match self {
      Value::Binary(ref data) => Ok(data),
      value => Err(KbinErrorKind::ValueTypeMismatch(StandardType::Binary, value.clone()).into()),
    }
  }

  pub fn as_str(&self) -> Result<&str, KbinError> {
    match self {
      Value::String(ref s) => Ok(s),
      value => Err(KbinErrorKind::ValueTypeMismatch(StandardType::String, value.clone()).into()),
    }
  }

  pub fn as_string(self) -> Result<String, KbinError> {
    match self {
      Value::String(s) => Ok(s),
      value => Err(KbinErrorKind::ValueTypeMismatch(StandardType::String, value).into()),
    }
  }

  pub fn as_attribute(self) -> Result<String, KbinError> {
    match self {
      Value::Attribute(s) => Ok(s),
      value => Err(KbinErrorKind::ValueTypeMismatch(StandardType::Attribute, value).into()),
    }
  }

  pub fn as_binary(&self) -> Result<&[u8], KbinError> {
    match self {
      Value::Binary(ref data) => Ok(data),
      value => Err(KbinErrorKind::ValueTypeMismatch(StandardType::Binary, value.clone()).into()),
    }
  }

  pub fn as_array(&self) -> Result<&ValueArray, KbinError> {
    match self {
      Value::ArrayNew(ref values) => Ok(values),
      value => Err(KbinErrorKind::ExpectedValueArray(value.clone()).into()),
    }
  }

  pub fn into_binary(self) -> Result<Vec<u8>, KbinError> {
    match self {
      Value::Binary(data) => Ok(data),
      value => Err(KbinErrorKind::ValueTypeMismatch(StandardType::Binary, value).into()),
    }
  }
}

impl TryFrom<Value> for Vec<Value> {
  type Error = KbinError;

  fn try_from(value: Value) -> Result<Self, Self::Error> {
    match value {
      Value::Array(_, values) => Ok(values),
      value => Err(KbinErrorKind::ExpectedValueArray(value).into()),
    }
  }
}

impl TryFrom<Value> for Vec<u8> {
  type Error = KbinError;

  fn try_from(value: Value) -> Result<Self, Self::Error> {
    // An array of unsigned 8-bit integers can either be `Binary` or a literal
    // array of unsigned 8-bit integers.
    match value {
      Value::Binary(data) => Ok(data),
      Value::Array(node_type, values) => {
        if node_type != StandardType::U8 {
          return Err(KbinErrorKind::ValueTypeMismatch(StandardType::U8, Value::Array(node_type, values)).into());
        }
        values.iter().map(Value::as_u8).collect()
      },
      value => Err(KbinErrorKind::ValueTypeMismatch(StandardType::Binary, value).into()),
    }
  }
}

impl TryFrom<&Value> for Vec<u8> {
  type Error = KbinError;

  fn try_from(value: &Value) -> Result<Self, Self::Error> {
    match value {
      Value::Binary(ref data) => Ok(data.to_vec()),
      Value::Array(ref node_type, ref values) => {
        if *node_type != StandardType::U8 {
          return Err(KbinErrorKind::ValueTypeMismatch(StandardType::U8, value.clone()).into());
        }
        values.iter().map(Value::as_u8).collect()
      },
      value => Err(KbinErrorKind::ValueTypeMismatch(StandardType::Binary, value.clone()).into()),
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
          Value::Array(ref node_type, ref a) => if f.alternate() {
            write!(f, "Array({:?}, {:#?})", node_type, a)
          } else {
            write!(f, "Array({:?}, {:?})", node_type, a)
          },
          Value::ArrayNew(ref value) => if f.alternate() {
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

/// A separate wrapper struct so `Value::Array` can be formatted by
/// `<Value as fmt::Display>` and `Value::array_as_string`
struct BorrowedValueArray<'a>(&'a [Value]);

impl<'a> fmt::Display for BorrowedValueArray<'a> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    for (i, v) in self.0.iter().enumerate() {
      if i > 0 {
        f.write_str(" ")?;
      }
      fmt::Display::fmt(v, f)?;
    }
    Ok(())
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
          Value::Array(_, values) => BorrowedValueArray(&values).fmt(f),
        }
      };
    }

    display_value! {
      simple: [
        S8, U8, S16, U16, S32, U32, S64, U64,
        String, Ip4, Time, Attribute,
        ArrayNew
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
