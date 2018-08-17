use byteorder::{BigEndian, WriteBytesExt};
use failure::ResultExt;
use serde::ser::{self, Impossible, Serialize};

use error::{Error, KbinErrorKind};
use node_types::StandardType;

pub struct BufferSerializer {
  buffer: Vec<u8>,
}

impl BufferSerializer {
  pub fn new() -> Self {
    Self {
      buffer: Vec::new(),
    }
  }

  #[inline]
  pub fn get_ref(&self) -> &[u8] {
    &self.buffer
  }

  #[inline]
  pub fn into_inner(self) -> Vec<u8> {
    self.buffer
  }
}

macro_rules! ser_type {
  (byte; $inner_type:ident, $method:ident, $standard_type:ident $($cast:tt)*) => {
    fn $method(self, value: $inner_type) -> Result<Self::Ok, Self::Error> {
      let node_type = StandardType::$standard_type;

      trace!("BufferSerializer::{}(node_type: {}, value: {})", stringify!($method), stringify!($standard_type), value);
      self.buffer.write_u8(value $($cast)*).context(KbinErrorKind::DataWrite(node_type.name))?;

      Ok(node_type)
    }
  };
  (large; $inner_type:ident, $method:ident, $write_method:ident, $standard_type:ident $($cast:tt)*) => {
    fn $method(self, value: $inner_type) -> Result<Self::Ok, Self::Error> {
      trace!(concat!("BufferSerializer::{}(node_type: {}, value: {})"), stringify!($method), stringify!($standard_type), value);

      //self.buffer.push(Value::$standard_type(value));
      let node_type = StandardType::$standard_type;
      self.buffer.$write_method::<BigEndian>(value $($cast)*).context(KbinErrorKind::DataWrite(node_type.name))?;

      Ok(node_type)
    }
  }
}

impl<'a> ser::Serializer for &'a mut BufferSerializer {
  type Ok = StandardType;
  type Error = Error;

  type SerializeSeq = Impossible<Self::Ok, Self::Error>;
  type SerializeTuple = Impossible<Self::Ok, Self::Error>;
  type SerializeTupleStruct = Impossible<Self::Ok, Self::Error>;
  type SerializeTupleVariant = Impossible<Self::Ok, Self::Error>;
  type SerializeMap = Impossible<Self::Ok, Self::Error>;
  type SerializeStruct = Impossible<Self::Ok, Self::Error>;
  type SerializeStructVariant = Impossible<Self::Ok, Self::Error>;

  fn is_human_readable(&self) -> bool {
    false
  }

  ser_type!(byte; bool, serialize_bool, Boolean as u8);
  ser_type!(byte; u8, serialize_u8, U8);
  ser_type!(byte; i8, serialize_i8, S8 as u8);
  ser_type!(large; u16, serialize_u16, write_u16, U16);
  ser_type!(large; i16, serialize_i16, write_i16, S16);
  ser_type!(large; u32, serialize_u32, write_u32, U32);
  ser_type!(large; i32, serialize_i32, write_i32, S32);
  ser_type!(large; u64, serialize_u64, write_u64, U64);
  ser_type!(large; i64, serialize_i64, write_i64, S64);
  ser_type!(large; f32, serialize_f32, write_f32, Float);
  ser_type!(large; f64, serialize_f64, write_f64, Double);

  fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
    trace!("BufferSerializer::serialize_char(value: {})", value);
    Err(Error::StaticMessage("char not supported"))
  }

  fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
    trace!("BufferSerializer::serialize_str(value: {})", value);
    Err(Error::StaticMessage("str not supported"))
  }

  // Binary data is handled separately from other array types.
  // Binary data should also be the only element of its node.
  fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
    trace!("BufferSerializer::serialize_bytes(value: {:02x?})", value);
    Err(Error::StaticMessage("bytes not supported"))
  }

  fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
    trace!("BufferSerializer::serialize_none()");
    Err(Error::StaticMessage("option not supported"))
  }

  fn serialize_some<T>(self, _v: &T) -> Result<Self::Ok, Self::Error>
    where T: ?Sized + Serialize
  {
    trace!("BufferSerializer::serialize_some()");
    Err(Error::StaticMessage("option not supported"))
  }

  fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
    trace!("BufferSerializer::serialize_unit()");
    Err(Error::StaticMessage("unit not supported"))
  }

  fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
    trace!("BufferSerializer::serialize_unit_struct(name: {})", name);
    Err(Error::StaticMessage("unit struct not supported"))
  }

  fn serialize_unit_variant(self, name: &'static str, variant_index: u32, variant: &'static str) -> Result<Self::Ok, Self::Error> {
    trace!("BufferSerializer::serialize_unit_variant(name: {}, variant_index: {}, variant: {})", name, variant_index, variant);
    Err(Error::StaticMessage("unit variant not supported"))
  }

  fn serialize_newtype_struct<T>(self, name: &'static str, _value: &T) -> Result<Self::Ok, Self::Error>
    where T: ?Sized + Serialize
  {
    trace!("BufferSerializer::serialize_newtype_struct(name: {})", name);
    Err(Error::StaticMessage("newtype struct not supported"))
  }

  fn serialize_newtype_variant<T>(self, name: &'static str, variant_index: u32, variant: &'static str, _value: &T) -> Result<Self::Ok, Self::Error>
    where T: ?Sized + Serialize
  {
    trace!("BufferSerializer::serialize_newtype_variant(name: {}, variant_index: {}, variant: {})", name, variant_index, variant);
    Err(Error::StaticMessage("newtype variant not supported"))
  }

  fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
    trace!("BufferSerializer::serialize_seq(len: {:?})", len);
    Err(Error::StaticMessage("sequences not supported"))
  }

  fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
    trace!("BufferSerializer::serialize_tuple(len: {})", len);
    Err(Error::StaticMessage("tuple not supported"))
  }

  fn serialize_tuple_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeTupleStruct, Self::Error> {
    trace!("BufferSerializer::serialize_tuple_struct(name: {}, len: {})", name, len);
    Err(Error::StaticMessage("tuple struct not supported"))
  }

  fn serialize_tuple_variant(self, name: &'static str, variant_index: u32, variant: &'static str, len: usize) -> Result<Self::SerializeTupleVariant, Self::Error> {
    trace!("BufferSerializer::serialize_tuple_variant(name: {}, variant_index: {}, variant: {}, len: {})", name, variant_index, variant, len);
    Err(Error::StaticMessage("tuple variant not supported"))
  }

  fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
    trace!("BufferSerializer::serialize_map(len: {:?})", len);
    Err(Error::StaticMessage("map not supported"))
  }

  fn serialize_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeStruct, Self::Error> {
    trace!("BufferSerializer::serialize_struct(name: {}, len: {})", name, len);
    Err(Error::StaticMessage("struct not supported"))
  }

  fn serialize_struct_variant(self, name: &'static str, variant_index: u32, variant: &'static str, len: usize) -> Result<Self::SerializeStructVariant, Self::Error> {
    trace!("BufferSerializer::serialize_struct_variant(name: {}, variant_index: {}, variant: {}, len: {})", name, variant_index, variant, len);
    Err(Error::StaticMessage("struct variant not supported"))
  }
}
