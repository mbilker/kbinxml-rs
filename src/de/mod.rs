use std::result::Result as StdResult;

use byteorder::{BigEndian, ByteOrder, ReadBytesExt};
use failure::ResultExt;
use serde::de::{self, Deserialize, Visitor};

use byte_buffer::ByteBufferRead;
use compression::Compression;
use encoding_type::EncodingType;
use error::{Error, KbinErrorKind};
use node_types::StandardType;
use sixbit::unpack_sixbit;
use super::{ARRAY_MASK, SIGNATURE, SIG_COMPRESSED};

mod seq;
mod structure;

use self::seq::Seq;
use self::structure::Struct;

pub type Result<T> = StdResult<T, Error>;

enum ReadMode {
  Single,
  Array,
}

pub struct Deserializer<'de> {
  encoding: EncodingType,

  read_mode: ReadMode,
  first_struct: bool,

  //node_buf_end: u64,
  node_buf: ByteBufferRead<&'de [u8]>,
  data_buf: ByteBufferRead<&'de [u8]>,
}

pub fn from_bytes<'a, T>(input: &'a [u8]) -> Result<T>
  where T: Deserialize<'a>
{
  let mut deserializer = Deserializer::new(input)?;
  let t = T::deserialize(&mut deserializer)?;
  Ok(t)
}

impl<'de> Deserializer<'de> {
  pub fn new(input: &'de [u8]) -> Result<Self> {
    // Node buffer starts from the beginning.
    // Data buffer starts later after reading `len_data`.
    let mut node_buf = ByteBufferRead::new(&input[..]);

    let signature = node_buf.read_u8().context(KbinErrorKind::HeaderRead("signature"))?;
    if signature != SIGNATURE {
      return Err(KbinErrorKind::HeaderValue("signature").into());
    }

    // TODO: support uncompressed
    let compress_byte = node_buf.read_u8().context(KbinErrorKind::HeaderRead("compression"))?;
    if compress_byte != SIG_COMPRESSED {
      return Err(KbinErrorKind::HeaderValue("compression").into());
    }

    let compressed = Compression::from_byte(compress_byte)?;

    let encoding_byte = node_buf.read_u8().context(KbinErrorKind::HeaderRead("encoding"))?;
    let encoding_negation = node_buf.read_u8().context(KbinErrorKind::HeaderRead("encoding negation"))?;
    let encoding = EncodingType::from_byte(encoding_byte)?;
    if encoding_negation != !encoding_byte {
      return Err(KbinErrorKind::HeaderValue("encoding negation").into());
    }

    info!("signature: 0x{:x}, compression: 0x{:x} ({:?}), encoding: 0x{:x} ({:?})", signature, compress_byte, compressed, encoding_byte, encoding);

    let len_node = node_buf.read_u32::<BigEndian>().context(KbinErrorKind::LenNodeRead)?;
    info!("len_node: {0} (0x{0:x})", len_node);

    // We have read 8 bytes so far, so offset the start of the data buffer from
    // the start of the input data.
    let data_buf_start = len_node + 8;
    let mut data_buf = ByteBufferRead::new(&input[(data_buf_start as usize)..]);

    let len_data = data_buf.read_u32::<BigEndian>().context(KbinErrorKind::LenDataRead)?;
    info!("len_data: {0} (0x{0:x})", len_data);

    //let node_buf_end = data_buf_start.into();

    Ok(Self {
      encoding,
      read_mode: ReadMode::Single,
      first_struct: true,
      //node_buf_end,
      node_buf,
      data_buf,
    })
  }

  fn read_node(&mut self) -> Result<StandardType> {
    let raw_node_type = self.node_buf.read_u8().context(KbinErrorKind::NodeTypeRead)?;
    let is_array = raw_node_type & ARRAY_MASK == ARRAY_MASK;
    let node_type = raw_node_type & !ARRAY_MASK;

    let xml_type = StandardType::from_u8(node_type);
    debug!("raw_node_type: {}, node_type: {:?} ({}), is_array: {}", raw_node_type, xml_type, node_type, is_array);

    Ok(xml_type)
  }

  fn read_name(&mut self) -> Result<String> {
    unpack_sixbit(&mut *self.node_buf).map_err(Error::from)
  }

  fn read_node_with_name(&mut self) -> Result<(StandardType, String)> {
    let node_type = self.read_node()?;
    let name = self.read_name()?;
    debug!("name: {}", name);

    Ok((node_type, name))
  }
}

macro_rules! de_type {
  (byte; $method:ident, $visit_method:ident, $standard_type:ident $($cast:tt)*) => {
    fn $method<V>(self, visitor: V) -> Result<V::Value>
      where V: Visitor<'de>
    {
      let value = match self.read_mode {
        ReadMode::Single => {
          self.data_buf.get_aligned(*StandardType::$standard_type)?[0] $($cast)*
        },
        ReadMode::Array => {
          self.data_buf.read_u8().context(KbinErrorKind::DataRead(1))? $($cast)*
        },
      };
      trace!(concat!("Deserializer::", stringify!($method), "() => value: {:?}"), value);

      visitor.$visit_method(value)
    }
  };
  (large; $method:ident, $visit_method:ident, $read_method:ident, $standard_type:ident) => {
    fn $method<V>(self, visitor: V) -> Result<V::Value>
      where V: Visitor<'de>
    {
      let value = match self.read_mode {
        ReadMode::Single => {
          let value = self.data_buf.get_aligned(*StandardType::$standard_type)?;
          BigEndian::$read_method(&value)
        },
        ReadMode::Array => {
          self.data_buf.$read_method::<BigEndian>().context(KbinErrorKind::DataRead(StandardType::$standard_type.size as usize))?
        },
      };
      trace!(concat!("Deserializer::", stringify!($method), "() => value: {:?}"), value);

      visitor.$visit_method(value)
    }
  }
}

macro_rules! implement_type {
  ($method:ident) => {
    fn $method<V>(self, _visitor: V) -> Result<V::Value>
      where V: Visitor<'de>
    {
      trace!("Deserializer::{}()", stringify!($method));
      unimplemented!();
    }
  }
}

impl<'de, 'a> de::Deserializer<'de> for &'a mut Deserializer<'de> {
  type Error = Error;

  fn is_human_readable(&self) -> bool {
    false
  }

  fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_any()");
    Err(KbinErrorKind::DataRead(1).into())
  }

  fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_bool()");

    let value = self.data_buf.get_aligned(*StandardType::Boolean)?[0];
    trace!("Deserializer::deserialize_bool() => value: {:?}", value);

    let value = match value {
      0x00 => false,
      0x01 => true,
      value => return Err(Error::Message(format!("invalid value for boolean: {0:?} (0x{0:x})", value))),
    };

    visitor.visit_bool(value)
  }

  de_type!(byte; deserialize_u8, visit_u8, U8);
  de_type!(byte; deserialize_i8, visit_i8, S8 as i8);
  de_type!(large; deserialize_u16, visit_u16, read_u16, U16);
  de_type!(large; deserialize_i16, visit_i16, read_i16, S16);
  de_type!(large; deserialize_u32, visit_u32, read_u32, U32);
  de_type!(large; deserialize_i32, visit_i32, read_i32, S32);
  de_type!(large; deserialize_u64, visit_u64, read_u64, U64);
  de_type!(large; deserialize_i64, visit_i64, read_i64, S64);
  de_type!(large; deserialize_f32, visit_f32, read_f32, Float);
  de_type!(large; deserialize_f64, visit_f64, read_f64, Double);
  implement_type!(deserialize_char);
  implement_type!(deserialize_str);

  fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_string()");

    visitor.visit_string(self.data_buf.read_str(self.encoding)?)
  }

  implement_type!(deserialize_bytes);
  implement_type!(deserialize_byte_buf);

  fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_option()");

    // A `None` value will not occur because it will not be present in the input data
    visitor.visit_some(self)
  }

  implement_type!(deserialize_unit);

  fn deserialize_unit_struct<V>(self, name: &'static str, _visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_unit_struct(name: {:?})", name);
    unimplemented!();
  }

  fn deserialize_newtype_struct<V>(self, name: &'static str, _visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_newtype_struct(name: {:?})", name);
    unimplemented!();
  }

  fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_seq()");

    // TODO: add size check against len
    let size = self.data_buf.read_u32::<BigEndian>().context(KbinErrorKind::ArrayLengthRead)?;
    debug!("Deserializer::deserialize_seq() => read array size: {}", size);

    // Changes to `self.read_mode` must stay here as `next_element_seed` is not
    // called past the length of the array to reset the read mode
    self.read_mode = ReadMode::Array;
    let value = visitor.visit_seq(Seq::new(self, size as usize))?;
    self.read_mode = ReadMode::Single;
    self.data_buf.realign_reads(None)?;

    Ok(value)
  }

  fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_tuple(len: {})", len);

    self.deserialize_seq(visitor)
  }

  fn deserialize_tuple_struct<V>(self, name: &'static str, len: usize, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_tuple_struct(name: {:?}, len: {})", name, len);

    self.read_mode = ReadMode::Array;
    let value = visitor.visit_seq(Seq::new(self, len))?;
    self.read_mode = ReadMode::Single;
    self.data_buf.realign_reads(None)?;

    Ok(value)
  }

  fn deserialize_map<V>(self, _visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_map()");
    unimplemented!();
  }

  fn deserialize_struct<V>(self, name: &'static str, fields: &'static [&'static str], visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_struct(name: {:?})", name);
    trace!("Deserializer::deserialize_struct() => fields: {:?}", fields);

    // The `NodeStart` event is consumed by `deserialize_identifier` when
    // reading the parent struct, don't consume the next event.
    if self.first_struct {
      let (node_type, name) = self.read_node_with_name()?;
      debug!("node_type: {:?}, name: {:?}", node_type, name);

      // Sanity check
      if node_type != StandardType::NodeStart {
        return Err(KbinErrorKind::TypeMismatch(*StandardType::NodeStart, *node_type).into());
      }
    }
    self.first_struct = false;

    let value = visitor.visit_map(Struct::new(self, fields))?;

    Ok(value)
  }

  fn deserialize_enum<V>(self, name: &'static str, variants: &'static [&'static str], _visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_enum(name: {:?}, variants: {:?})", name, variants);
    unimplemented!();
  }

  fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_identifier()");

    // Do not use `deserialize_string`! That reads from the data buffer and
    // this reads a sixbit string from the node buffer
    visitor.visit_string(self.read_name()?)
  }

  implement_type!(deserialize_ignored_any);
}
