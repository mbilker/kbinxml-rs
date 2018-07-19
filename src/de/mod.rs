use std::result::Result as StdResult;

use byteorder::{BigEndian, ByteOrder, ReadBytesExt};
use failure::ResultExt;
use serde::de::{self, Deserialize, Visitor};

use error::{Error, KbinErrorKind};
use node_types::StandardType;
use reader::Reader;

mod map;
mod seq;
mod structure;

use self::map::Map;
use self::seq::Seq;
use self::structure::Struct;

pub type Result<T> = StdResult<T, Error>;

enum ReadMode {
  Single,
  Array,
}

pub struct Deserializer<'de> {
  read_mode: ReadMode,
  node_stack: Vec<StandardType>,
  first_struct: bool,

  reader: Reader<'de>,
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
    let reader = Reader::new(input)?;

    Ok(Self {
      read_mode: ReadMode::Single,
      node_stack: Vec::new(),
      first_struct: true,
      reader,
    })
  }

  fn read_node_with_name(&mut self) -> Result<(StandardType, bool, String)> {
    let (node_type, is_array) = self.reader.read_node_type()?;
    let name = self.reader.read_node_identifier()?;
    debug!("name: {}", name);

    Ok((node_type, is_array, name))
  }
}

macro_rules! de_type {
  (byte; $method:ident, $visit_method:ident, $standard_type:ident $($cast:tt)*) => {
    fn $method<V>(self, visitor: V) -> Result<V::Value>
      where V: Visitor<'de>
    {
      let value = match self.read_mode {
        ReadMode::Single => {
          self.reader.data_buf.get_aligned(*StandardType::$standard_type)?[0] $($cast)*
        },
        ReadMode::Array => {
          self.reader.read_u8().context(KbinErrorKind::DataRead(1))? $($cast)*
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
          let value = self.reader.data_buf.get_aligned(*StandardType::$standard_type)?;
          BigEndian::$read_method(&value)
        },
        ReadMode::Array => {
          self.reader.data_buf.$read_method::<BigEndian>().context(KbinErrorKind::DataRead(StandardType::$standard_type.size as usize))?
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

  fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_any()");

    let (node_type, _is_array) = self.reader.last_node_type().ok_or(KbinErrorKind::InvalidState)?;
    debug!("Deserializer::deserialize_any() => node_type: {:?}", node_type);

    let value = match node_type {
      StandardType::Attribute |
      StandardType::String => self.deserialize_string(visitor),
      StandardType::Binary => self.deserialize_bytes(visitor),
      StandardType::U8 => self.deserialize_u8(visitor),
      StandardType::U16 => self.deserialize_u16(visitor),
      StandardType::U32 => self.deserialize_u32(visitor),
      StandardType::U64 => self.deserialize_u64(visitor),
      StandardType::S8 => self.deserialize_i8(visitor),
      StandardType::S16 => self.deserialize_i16(visitor),
      StandardType::S32 => self.deserialize_i32(visitor),
      StandardType::S64 => self.deserialize_i64(visitor),
      StandardType::NodeStart => self.deserialize_identifier(visitor),
      StandardType::NodeEnd => visitor.visit_none(),
      _ => unimplemented!(),
    };
    value
  }

  fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_bool()");

    let value = self.reader.data_buf.get_aligned(*StandardType::Boolean)?[0];
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

    visitor.visit_string(self.reader.read_string()?)
  }

  fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_bytes()");

    visitor.visit_bytes(self.reader.read_bytes()?)
  }

  fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_byte_buf()");

    visitor.visit_byte_buf(self.reader.read_bytes()?.to_vec())
  }

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

    let (node_type, _) = self.reader.last_node_type().ok_or(KbinErrorKind::InvalidState)?;

    // If the last node type on the stack is a `NodeStart` then we are likely
    // collecting a list of structs
    let value = if node_type == StandardType::NodeStart {
      visitor.visit_seq(Seq::new(self, None)?)?
    } else {
      // TODO: add size check against len
      let size = self.reader.read_u32().context(KbinErrorKind::ArrayLengthRead)?;
      debug!("Deserializer::deserialize_seq() => read array size: {}", size);

      // Changes to `self.read_mode` must stay here as `next_element_seed` is not
      // called past the length of the array to reset the read mode
      self.read_mode = ReadMode::Array;
      let value = visitor.visit_seq(Seq::new(self, Some(size as usize))?)?;
      self.read_mode = ReadMode::Single;
      self.reader.data_buf.realign_reads(None)?;

      value
    };

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
    let value = visitor.visit_seq(Seq::new(self, Some(len))?)?;
    self.read_mode = ReadMode::Single;
    self.reader.data_buf.realign_reads(None)?;

    Ok(value)
  }

  fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_map()");

    let (node_type, _, name) = self.read_node_with_name()?;
    debug!("Deserializer::deserialize_map() => node_type: {:?}, name: {:?}", node_type, name);

    visitor.visit_map(Map::new(self))
  }

  fn deserialize_struct<V>(self, name: &'static str, fields: &'static [&'static str], visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_struct(name: {:?}, fields: {:?})", name, fields);

    // The `NodeStart` event is consumed by `deserialize_identifier` when
    // reading the parent struct, don't consume the next event.
    if self.first_struct {
      let (node_type, _, name) = self.read_node_with_name()?;
      debug!("Deserializer::deserialize_struct() => node_type: {:?}, name: {:?}, last identifier: {:?}", node_type, name, self.reader.last_identifier());

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
    let (node_type, _) = self.reader.last_node_type().ok_or(KbinErrorKind::InvalidState)?;
    trace!("Deserializer::deserialize_identifier() => last node type: {:?}", node_type);

    // Prefix Attribute node identifier's with "attr_" to help the serializer
    let name = match (node_type, self.reader.read_node_identifier()?) {
      (StandardType::Attribute, name) => format!("attr_{}", name),
      (StandardType::NodeStart, name) => {
        self.first_struct = false;
        name
      },
      (_, name) => name,
    };
    debug!("Deserializer::deserialize_identifier() => name: '{}'", name);

    // Do not use `deserialize_string`! That reads from the data buffer and
    // this reads a sixbit string from the node buffer
    visitor.visit_string(name)
  }

  fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_ignored_any()");

    self.deserialize_any(visitor)
  }
}
