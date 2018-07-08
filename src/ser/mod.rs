use std::io::{Cursor, Write};
use std::result::Result as StdResult;

use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use failure::ResultExt;
use serde::ser::{self, Impossible, Serialize};

use byte_buffer::ByteBufferWrite;
use encoding_type::EncodingType;
use node_types::StandardType;
use super::error::{KbinError, KbinErrorKind};

mod error;
mod structure;
mod tuple;

use self::error::Error;
use self::structure::Struct;
use self::tuple::Tuple;

const SIGNATURE: u8 = 0xA0;

const SIG_COMPRESSED: u8 = 0x42;

const ARRAY_MASK: u8 = 1 << 6; // 1 << 6 = 64

pub type Result<T> = StdResult<T, Error>;

// Writing arrays should not be aligned after each write. Buffer realignment
// should be performed after Writing a single value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WriteMode {
  Single,
  Array,
}

pub struct Serializer {
  encoding: EncodingType,

  hierarchy: Vec<&'static str>,
  write_mode: WriteMode,

  node_buf: ByteBufferWrite,
  data_buf: ByteBufferWrite,
}

#[derive(Debug)]
pub struct TypeHint {
  node_type: StandardType,
  is_array: bool,
  count: usize,
}

impl TypeHint {
  fn from_type(node_type: StandardType) -> Self {
    Self {
      node_type,
      is_array: false,
      count: 1,
    }
  }
}

pub fn to_bytes<T>(value: &T) -> Result<Vec<u8>>
  where T: Serialize
{
  let mut serializer = Serializer {
    encoding: EncodingType::SHIFT_JIS,
    hierarchy: Vec::new(),
    write_mode: WriteMode::Single,
    node_buf: ByteBufferWrite::new(Vec::new()),
    data_buf: ByteBufferWrite::new(Vec::new()),
  };
  value.serialize(&mut serializer)?;

  let output = serializer.finalize()?;
  Ok(output)
}

impl Serializer {
  fn finalize(mut self) -> StdResult<Vec<u8>, KbinError> {
    let mut header = Cursor::new(Vec::with_capacity(8));
    header.write_u8(SIGNATURE).context(KbinErrorKind::HeaderWrite("signature"))?;
    header.write_u8(SIG_COMPRESSED).context(KbinErrorKind::HeaderWrite("compression"))?;

    let encoding = self.encoding.to_byte();
    header.write_u8(encoding).context(KbinErrorKind::HeaderWrite("encoding"))?;
    header.write_u8(0xFF ^ encoding).context(KbinErrorKind::HeaderWrite("encoding negation"))?;

    self.node_buf.write_u8(StandardType::FileEnd.id | ARRAY_MASK).context(KbinErrorKind::DataWrite("file end"))?;
    self.node_buf.realign_writes(None)?;

    let mut output = header.into_inner();

    let node_buf = self.node_buf.into_inner();
    output.write_u32::<BigEndian>(node_buf.len() as u32).context(KbinErrorKind::HeaderWrite("node buffer length"))?;
    output.extend_from_slice(&node_buf);

    let data_buf = self.data_buf.into_inner();
    output.write_u32::<BigEndian>(data_buf.len() as u32).context(KbinErrorKind::HeaderWrite("data buffer length"))?;
    output.extend_from_slice(&data_buf);

    Ok(output)
  }
}

// `straight_impl` passes a single element array to `write_aligned` where
// `primitive_impl` will use `BigEndian` to populate a multi-element array for
// `write_aligned`
macro_rules! byte_impl {
  ($inner_type:ident, $method:ident, $standard_type:ident $($cast:tt)*) => {
    fn $method(self, value: $inner_type) -> Result<Self::Ok> {
      debug!(concat!(stringify!($method), " => value: {}"), value);

      let node_type = StandardType::$standard_type;
      match self.write_mode {
        WriteMode::Single => {
          let value = value $($cast)*;
          self.data_buf.write_aligned(*node_type, &[value])?;
        },
        WriteMode::Array => {
          self.data_buf.write_u8(value $($cast)*).context(KbinErrorKind::DataWrite(node_type.name))?;
        }
      };

      Ok(Some(TypeHint::from_type(node_type)))
    }
  }
}

macro_rules! primitive_impl {
  ($inner_type:ident, $method:ident, $write_method:ident, $standard_type:ident $($cast:tt)*) => {
    fn $method(self, value: $inner_type) -> Result<Self::Ok> {
      debug!(concat!(stringify!($method), " => value: {}"), value);

      let node_type = StandardType::$standard_type;
      match self.write_mode {
        WriteMode::Single => {
          let mut buf = [0; ::std::mem::size_of::<$inner_type>()];
          BigEndian::$write_method(&mut buf, value);
          self.data_buf.write_aligned(*node_type, &buf)?;
        },
        WriteMode::Array => {
          self.data_buf.$write_method::<BigEndian>(value $($cast)*).context(KbinErrorKind::DataWrite(node_type.name))?;
        }
      };

      Ok(Some(TypeHint::from_type(node_type)))
    }
  }
}

impl<'a> ser::Serializer for &'a mut Serializer {
  type Ok = Option<TypeHint>;
  type Error = Error;

  type SerializeSeq = Tuple<'a>;
  type SerializeTuple = Tuple<'a>;
  type SerializeTupleStruct = Impossible<Self::Ok, Self::Error>;
  type SerializeTupleVariant = Impossible<Self::Ok, Self::Error>;
  type SerializeMap = Self;
  type SerializeStruct = Struct<'a>;
  type SerializeStructVariant = Impossible<Self::Ok, Self::Error>;

  byte_impl!(bool, serialize_bool, Boolean as u8);
  byte_impl!(u8, serialize_u8, U8);
  byte_impl!(i8, serialize_i8, S8 as u8);
  primitive_impl!(u16, serialize_u16, write_u16, U16);
  primitive_impl!(i16, serialize_i16, write_i16, S16);
  primitive_impl!(u32, serialize_u32, write_u32, U32);
  primitive_impl!(i32, serialize_i32, write_i32, S32);
  primitive_impl!(u64, serialize_u64, write_u64, U64);
  primitive_impl!(i64, serialize_i64, write_i64, S64);
  primitive_impl!(f32, serialize_f32, write_f32, Float);
  primitive_impl!(f64, serialize_f64, write_f64, Double);

  fn serialize_char(self, value: char) -> Result<Self::Ok> {
    debug!("serialize_char => value: {}", value);
    self.data_buf.write_str(self.encoding, &value.to_string())?;

    Ok(Some(TypeHint::from_type(StandardType::String)))
  }

  fn serialize_str(self, value: &str) -> Result<Self::Ok> {
    debug!("serialize_str => value: {}", value);
    self.data_buf.write_str(self.encoding, value)?;

    Ok(Some(TypeHint::from_type(StandardType::String)))
  }

  // Binary data is handled separately from other array types.
  // Binary data should also be the only element of its node.
  fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok> {
    debug!("serialize_bytes => value: {:02x?}", value);
    let node_type = StandardType::Binary;
    let size = (value.len() as u32) * (node_type.size as u32);
    self.data_buf.write_u32::<BigEndian>(size).context(KbinErrorKind::DataWrite("binary node size"))?;
    self.data_buf.write_all(value).context(KbinErrorKind::DataWrite("binary"))?;
    self.data_buf.realign_writes(None)?;

    Ok(Some(TypeHint::from_type(node_type)))
  }

  // TODO: Figure out a good way to serialize this
  fn serialize_none(self) -> Result<Self::Ok> {
    debug!("serialize_none");
    Ok(None)
  }

  fn serialize_some<T>(self, v: &T) -> Result<Self::Ok>
    where T: ?Sized + Serialize
  {
    debug!("serialize_some");
    let hint = v.serialize(&mut *self)?;
    Ok(hint)
  }

  // TODO: Figure out a good way to serialize this
  fn serialize_unit(self) -> Result<Self::Ok> {
    debug!("serialize_unit");
    Ok(None)
  }

  fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok> {
    debug!("serialize_unit_struct => name: {}", name);
    let hint = name.serialize(&mut *self)?;
    Ok(hint)
  }

  fn serialize_unit_variant(self, name: &'static str, variant_index: u32, variant: &'static str) -> Result<Self::Ok> {
    debug!("serialize_unit_variant => name: {}, variant_index: {}, variant: {}", name, variant_index, variant);
    let hint = variant.serialize(&mut *self)?;
    Ok(hint)
  }

  fn serialize_newtype_struct<T>(self, name: &'static str, value: &T) -> Result<Self::Ok>
    where T: ?Sized + Serialize
  {
    debug!("serialize_newtype_struct => name: {}", name);
    let hint = value.serialize(&mut *self)?;
    Ok(hint)
  }

  fn serialize_newtype_variant<T>(self, name: &'static str, variant_index: u32, variant: &'static str, value: &T) -> Result<Self::Ok>
    where T: ?Sized + Serialize
  {
    debug!("serialize_newtype_variant => name: {}, variant_index: {}, variant: {}", name, variant_index, variant);
    variant.serialize(&mut *self)?;
    let hint = value.serialize(&mut *self)?.map(|mut hint| {
      hint.is_array = false;
      hint
    });
    Ok(hint)
  }

  fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
    debug!("serialize_seq => len: {:?}", len);
    let len = len.ok_or(Error::Message("unsized sequences not supported".to_string()))?;
    Ok(Tuple::new(self, len))
  }

  fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
    debug!("serialize_tuple => len: {}", len);
    Ok(Tuple::new(self, len))
  }

  fn serialize_tuple_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeTupleStruct> {
    debug!("serialize_tuple_struct => name: {}, len: {}", name, len);
    Err(Error::Message("tuple struct not supported".to_string()))
  }

  fn serialize_tuple_variant(self, name: &'static str, variant_index: u32, variant: &'static str, len: usize) -> Result<Self::SerializeTupleVariant> {
    debug!("serialize_tuple_variant => name: {}, variant_index: {}, variant: {}, len: {}", name, variant_index, variant, len);
    Err(Error::Message("tuple variant not supported".to_string()))
  }

  fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
    debug!("serialize_map => len: {:?}", len);
    Ok(self)
  }

  fn serialize_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeStruct> {
    debug!("serialize_struct => name: {}, len: {}", name, len);

    Struct::new(self, name, len)
  }

  fn serialize_struct_variant(self, name: &'static str, variant_index: u32, variant: &'static str, len: usize) -> Result<Self::SerializeStructVariant> {
    debug!("serialize_struct_variant => name: {}, variant_index: {}, variant: {}, len: {}", name, variant_index, variant, len);
    Err(Error::Message("struct variant not supported".to_string()))
  }
}

impl<'a> ser::SerializeMap for &'a mut Serializer {
  type Ok = Option<TypeHint>;
  type Error = Error;

  fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where T: ?Sized + Serialize
  {
    debug!("SerializeMap: serialize_key");
    let hint = key.serialize(&mut **self)?;
    debug!("SerializeMap: serialize_key, hint: {:?}", hint);
    Ok(())
  }

  fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where T: ?Sized + Serialize
  {
    debug!("SerializeMap: serialize_value");
    let hint = value.serialize(&mut **self)?;
    debug!("SerializeMap: serialize_value, hint: {:?}", hint);
    Ok(())
  }

  fn end(self) -> Result<Self::Ok> {
    debug!("SerializeMap: end");
    Ok(None)
  }
}
