use std::io::{Cursor, Write};
use std::mem;
use std::result::Result as StdResult;

use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use failure::ResultExt;
use serde::ser::{self, Impossible, Serialize};

use byte_buffer::ByteBufferWrite;
use encoding_type::EncodingType;
use node_types::StandardType;
use sixbit::pack_sixbit;
use super::error::{KbinError, KbinErrorKind};

mod error;
mod tuple;

use self::error::Error;
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

  pub(crate) write_mode: WriteMode,

  pub(crate) node_buf: ByteBufferWrite,
  pub(crate) data_buf: ByteBufferWrite,
}

#[derive(Debug)]
pub struct TypeHint {
  node_type: StandardType,
  is_array: bool,
  count: usize,
}

impl TypeHint {
  /*
  fn new(node_type: StandardType, is_array: bool, count: usize) -> Self {
    Self { node_type, is_array, count }
  }
  */

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
  type SerializeStruct = Self;
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

  /*
  fn serialize_bool(self, value: bool) -> Result<Self::Ok> {
    debug!("serialize_bool => value: {}", value);
    let value = value as u8;

    let node_type = StandardType::Boolean;
    match self.write_mode {
      WriteMode::Single => self.data_buf.write_aligned(*node_type, &[value])
        .context(KbinErrorKind::DataWrite("bool"))?,
      WriteMode::Array => self.data_buf.write_u8(value)
        .context(KbinErrorKind::DataWrite("bool"))?,
    };

    Ok(TypeHint::from_type(node_type))
  }

  fn serialize_u8(self, value: u8) -> Result<Self::Ok> {
    debug!("serialize_u8 => value: {}", value);

    let node_type = StandardType::U8;
    match self.write_mode {
      WriteMode::Single => self.data_buf.write_aligned(*node_type, &[value])
        .context(KbinErrorKind::DataWrite("bool"))?,
      WriteMode::Array => self.data_buf.write_u8(value)
        .context(KbinErrorKind::DataWrite("bool"))?,
    };

    Ok(TypeHint::from_type(node_type))
  }

  fn serialize_i8(self, value: i8) -> Result<Self::Ok> {
    debug!("serialize_i8 => value: {}", value);
    let hint = self.write(StandardType::S8, &[value as u8]).context(KbinErrorKind::DataWrite("i8"))?;
    Ok(hint)
  }

  fn serialize_u16(self, value: u16) -> Result<Self::Ok> {
    debug!("serialize_u16 => value: {}", value);
    self.data_buf.write_u16::<BigEndian>(value).context(KbinErrorKind::DataWrite("u16"))?;

    Ok(TypeHint::from_type(StandardType::U16))
  }

  fn serialize_i16(self, value: i16) -> Result<Self::Ok> {
    debug!("serialize_i16 => value: {}", value);

    let node_type = StandardType::S16;
    match self.write_mode {
      WriteMode::Single => {
        let mut buf = [0; 2];
        BigEndian::write_i16(&mut buf, value);
        self.data_buf.write_aligned(*node_type, &buf)?;
      },
      WriteMode::Array => {
        self.data_buf.write_i16::<BigEndian>(value).context(KbinErrorKind::DataWrite(node_type.name))?;
      }
    };

    Ok(TypeHint::from_type(node_type))
  }

  fn serialize_u32(self, value: u32) -> Result<Self::Ok> {
    debug!("serialize_u32 => value: {}", value);
    self.data_buf.write_u32::<BigEndian>(value).context(KbinErrorKind::DataWrite("u32"))?;

    Ok(TypeHint::from_type(StandardType::U32))
  }

  fn serialize_i32(self, value: i32) -> Result<Self::Ok> {
    debug!("serialize_i32 => value: {}", value);
    self.data_buf.write_i32::<BigEndian>(value).context(KbinErrorKind::DataWrite("i32"))?;

    Ok(TypeHint::from_type(StandardType::S32))
  }

  fn serialize_u64(self, value: u64) -> Result<Self::Ok> {
    debug!("serialize_u64 => value: {}", value);
    self.data_buf.write_u64::<BigEndian>(value).context(KbinErrorKind::DataWrite("u64"))?;

    Ok(TypeHint::from_type(StandardType::U64))
  }

  fn serialize_i64(self, value: i64) -> Result<Self::Ok> {
    debug!("serialize_i64 => value: {}", value);
    self.data_buf.write_i64::<BigEndian>(value).context(KbinErrorKind::DataWrite("i64"))?;

    Ok(TypeHint::from_type(StandardType::S64))
  }

  fn serialize_f32(self, value: f32) -> Result<Self::Ok> {
    debug!("serialize_f32 => value: {}", value);
    self.data_buf.write_f32::<BigEndian>(value).context(KbinErrorKind::DataWrite("f32"))?;

    Ok(TypeHint::from_type(StandardType::Float))
  }

  fn serialize_f64(self, value: f64) -> Result<Self::Ok> {
    debug!("serialize_f64 => value: {}", value);
    self.data_buf.write_f64::<BigEndian>(value).context(KbinErrorKind::DataWrite("f64"))?;

    Ok(Some(TypeHint::from_type(StandardType::Double)))
  }
  */

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

    let node_type = StandardType::NodeStart;
    self.node_buf.write_u8(node_type.id).context(KbinErrorKind::DataWrite(node_type.name))?;
    pack_sixbit(&mut *self.node_buf, name)?;

    Ok(self)
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

impl<'a> ser::SerializeStruct for &'a mut Serializer {
  type Ok = Option<TypeHint>;
  type Error = Error;

  fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where T: ?Sized + Serialize
  {
    let size = mem::size_of_val(value);
    debug!("SerializeStruct: serialize_field, key: {}, value size: {}", key, size);

    let hint = value.serialize(&mut **self)?.ok_or(KbinErrorKind::MissingTypeHint)?;
    let array_mask = if hint.is_array { ARRAY_MASK } else { 0 };
    debug!("SerializeStruct: serialize_field, key: {}, hint: {:?}", key, hint);

    self.node_buf.write_u8(hint.node_type.id | array_mask).context(KbinErrorKind::DataWrite(hint.node_type.name))?;
    pack_sixbit(&mut *self.node_buf, key)?;

    // TODO: Make sure this does not prematurely end nodes
    self.node_buf.write_u8(StandardType::NodeEnd.id | ARRAY_MASK).context(KbinErrorKind::DataWrite("node end"))?;

    Ok(())
  }

  fn end(self) -> Result<Self::Ok> {
    debug!("SerializeStruct: end");
    self.node_buf.write_u8(StandardType::NodeEnd.id | ARRAY_MASK).context(KbinErrorKind::DataWrite("node end"))?;

    trace!("SerializeStruct::end() => node_buf: {:02x?}", self.node_buf.get_ref());

    Ok(Some(TypeHint::from_type(StandardType::NodeStart)))
  }
}