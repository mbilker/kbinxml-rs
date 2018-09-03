use std::result::Result as StdResult;

use byteorder::{BigEndian, ByteOrder, ReadBytesExt};
use failure::ResultExt;
use serde::de::{self, Deserialize, DeserializeSeed, IntoDeserializer, Visitor};

use error::{Error, KbinErrorKind};
use node::{Marshal, Node};
use node_types::StandardType;
use reader::Reader;

mod custom;
mod node_contents;
mod seq;
mod structure;
mod tuple;

use self::custom::Custom;
use self::node_contents::NodeContents;
use self::seq::Seq;
use self::structure::Struct;
use self::tuple::TupleBytesDeserializer;

pub type Result<T> = StdResult<T, Error>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReadMode {
  Key,
  Single,
  Array,
}

pub struct Deserializer<'de> {
  read_mode: ReadMode,
  node_stack: Vec<(StandardType, bool)>,
  first_struct: bool,
  ignore_attributes: bool,

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
      ignore_attributes: true,
      reader,
    })
  }

  #[inline]
  fn node_stack_last(&self) -> Result<&(StandardType, bool)> {
    self.node_stack.last()
      .ok_or(KbinErrorKind::InvalidState.into())
  }

  #[inline]
  fn set_read_mode(&mut self, read_mode: ReadMode) -> ReadMode {
    let old_read_mode = self.read_mode;
    self.read_mode = read_mode;

    old_read_mode
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
        ReadMode::Key => return Err(KbinErrorKind::InvalidState.into()),
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
        ReadMode::Key => return Err(KbinErrorKind::InvalidState.into()),
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

impl<'de, 'a> de::Deserializer<'de> for &'a mut Deserializer<'de> {
  type Error = Error;

  fn is_human_readable(&self) -> bool {
    false
  }

  fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    let (node_type, is_array) = self.node_stack_last()
      .map(|x| *x)
      .or_else(|_| -> Result<_> {
        let node = self.reader.peek_node_type()?;
        self.node_stack.push(node);
        Ok(node)
      })?;
    trace!("Deserializer::deserialize_any(node_type: {:?}, is_array: {})", node_type, is_array);

    // Handle arrays if we are not in array reading mode
    if is_array {
      // `Ip4` handling handled by `deserialize_seq`
      match self.read_mode {
        ReadMode::Array => {},
        _ => return self.deserialize_seq(visitor),
      };
    }

    // Only deserialize identifiers in `Key` mode
    if self.read_mode == ReadMode::Key {
      return self.deserialize_identifier(visitor);
    }

    let value = match node_type {
      /*
      StandardType::Attribute => self.deserialize_string(visitor),
      StandardType::String => self.deserialize_string(visitor),
      StandardType::Binary => visitor.visit_bytes(self.reader.read_bytes()?),
      StandardType::U8 => self.deserialize_u8(visitor),
      StandardType::U16 => self.deserialize_u16(visitor),
      StandardType::U32 => self.deserialize_u32(visitor),
      StandardType::U64 => self.deserialize_u64(visitor),
      StandardType::S8 => self.deserialize_i8(visitor),
      StandardType::S16 => self.deserialize_i16(visitor),
      StandardType::S32 => self.deserialize_i32(visitor),
      StandardType::S64 => self.deserialize_i64(visitor),
      StandardType::Binary => visitor.visit_bytes(self.reader.read_bytes()?),
      StandardType::Ip4 => {
        let old_read_mode = self.set_read_mode(ReadMode::Array);
        let value = visitor.visit_enum(Custom::new(self, node_type))?;
        self.read_mode = old_read_mode;
        Ok(value)
      },
      StandardType::Boolean => self.deserialize_bool(visitor),
      */
      StandardType::NodeStart => {
        debug!("Deserializer::deserialize_any(node_type: {:?}, is_array: {}) => deserializing node", node_type, is_array);
        let node = Node::deserialize(self);
        debug!("Deserializer::deserialize_any(node_type: {:?}, is_array: {}) => node: {:?}", node_type, is_array, node);
        let marshal = Marshal::with_node(StandardType::NodeStart, node?);
        visitor.visit_newtype_struct(marshal.into_deserializer())
      },
      /*
      StandardType::NodeEnd => {
        // Move `deserialize_any` on to the next node
        let _ = self.reader.read_node_type()?;
        self.deserialize_any(visitor)
      },
      */
      _ => {
        let value = node_type.deserialize(self)?;
        debug!("Deserializer::deserialize_any(node_type: {:?}, is_array: {}) => value: {:?}", node_type, is_array, value);
        let marshal = Marshal::with_value(node_type, value);
        visitor.visit_newtype_struct(marshal.into_deserializer())
      },
    };
    value
  }

  fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_bool()");

    let value = match self.read_mode {
      ReadMode::Key => return Err(KbinErrorKind::InvalidState.into()),
      ReadMode::Single => self.reader.data_buf.get_aligned(*StandardType::Boolean)?[0],
      ReadMode::Array => self.reader.read_u8().context(KbinErrorKind::DataRead(1))?,
    };
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

  fn deserialize_char<V>(self, _visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_char()");
    Err(Error::StaticMessage("char deserialization is not supported"))
  }

  fn deserialize_str<V>(self, _visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_str()");
    Err(Error::StaticMessage("borrowed string deserialization is not supported"))
  }

  fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_string() => read_mode: {:?}", self.read_mode);
    match self.read_mode {
      ReadMode::Key => self.deserialize_identifier(visitor),
      _ => visitor.visit_string(self.reader.read_string()?),
    }
  }

  fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_bytes()");
    visitor.visit_borrowed_bytes(self.reader.read_bytes()?)
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

  fn deserialize_unit<V>(self, _visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_unit()");
    Err(Error::StaticMessage("unit deserialization is not supported"))
  }

  fn deserialize_unit_struct<V>(self, name: &'static str, _visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_unit_struct(name: {:?})", name);
    Err(Error::StaticMessage("unit struct deserialization is not supported"))
  }

  fn deserialize_newtype_struct<V>(self, name: &'static str, _visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_newtype_struct(name: {:?})", name);
    Err(Error::StaticMessage("newtype struct deserialization is not supported"))
  }

  fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_seq(read_mode: {:?})", self.read_mode);

    let (node_type, _) = *self.node_stack_last()?;

    let value = match node_type {
      // If the last node type on the stack is a `NodeStart` then we are likely
      // collecting a list of structs
      StandardType::NodeStart => visitor.visit_seq(Seq::new(self, None)?)?,

      // Bytes should be deserialized by `deserialize_bytes`
      StandardType::Binary => self.deserialize_bytes(visitor)?,

      _ => {
        // TODO: add size check against len
        let node_size = node_type.size * node_type.count;
        let size = self.reader.read_u32().context(KbinErrorKind::ArrayLengthRead)?;
        let arr_count = (size as usize) / node_size;
        debug!("Deserializer::deserialize_seq() => read array size: {}, arr_count: {}", size, arr_count);

        // Changes to `self.read_mode` must stay here as `next_element_seed` is not
        // called past the length of the array to reset the read mode
        let old_read_mode = self.set_read_mode(ReadMode::Array);
        let value = visitor.visit_seq(Seq::new(self, Some(arr_count))?)?;
        self.read_mode = old_read_mode;

        // Only realign after the outermost array finishes reading
        if self.read_mode == ReadMode::Single {
          self.reader.data_buf.realign_reads(None)?;
        }

        value
      },
    };

    Ok(value)
  }

  fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_tuple(len: {})", len);

    let (node_type, is_array) = *self.node_stack_last()?;
    debug!("Deserializer::deserialize_tuple(len: {}) => node_type: {:?}, is_array: {}", len, node_type, is_array);

    // Handle case where kbin has an array but the Serde output is using a
    // tuple
    if is_array && self.read_mode == ReadMode::Single {
      return self.deserialize_seq(visitor);
    }

    // Use `get_aligned` to avoid edge cases with the indexors
    if is_array {
      let old_read_mode = self.set_read_mode(ReadMode::Array);
      let value = visitor.visit_seq(Seq::new(self, Some(len))?)?;
      self.read_mode = old_read_mode;

      // Only realign after the outermost array finishes reading
      if self.read_mode == ReadMode::Single {
        self.reader.data_buf.realign_reads(None)?;
      }

      Ok(value)
    } else {
      let data = self.reader.data_buf.get_aligned(*node_type)?;
      debug!("Deserializer::deserialize_tuple(len: {}) => data: 0x{:02x?}", len, data);

      visitor.visit_seq(TupleBytesDeserializer::new(node_type, data))
    }
  }

  fn deserialize_tuple_struct<V>(self, name: &'static str, len: usize, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_tuple_struct(name: {:?}, len: {})", name, len);

    match name {
      "__key" => {
        self.ignore_attributes = false;

        let (node_type, _is_array) = self.reader.last_node_type().ok_or(KbinErrorKind::InvalidState)?;
        visitor.visit_enum(Custom::new(self, node_type))
      },
      "__value" => {
        let (node_type, _is_array) = self.reader.last_node_type().ok_or(KbinErrorKind::InvalidState)?;
        debug!("Deserializer::deserialize_tuple_struct(name: {:?}) => node_type: {:?}", name, node_type);

        let value = visitor.visit_map(NodeContents::new(self, node_type))?;
        self.ignore_attributes = true;

        Ok(value)
      },
      _ => {
        let old_read_mode = self.set_read_mode(ReadMode::Array);
        let value = visitor.visit_seq(Seq::new(self, Some(len))?)?;
        self.read_mode = old_read_mode;
        self.reader.data_buf.realign_reads(None)?;

        Ok(value)
      },
    }
  }

  fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_map()");

    // The `NodeStart` event is consumed by `deserialize_identifier` when
    // reading the parent struct, don't consume the next event.
    if self.first_struct {
      let (node_type, _, name) = self.read_node_with_name()?;
      debug!("Deserializer::deserialize_map() => node_type: {:?}, name: {:?}, last identifier: {:?}", node_type, name, self.reader.last_identifier());

      // Sanity check
      if node_type != StandardType::NodeStart {
        return Err(KbinErrorKind::TypeMismatch(*StandardType::NodeStart, *node_type).into());
      }
    }
    self.first_struct = false;

    visitor.visit_map(Struct::new(self))
  }

  fn deserialize_struct<V>(self, name: &'static str, fields: &'static [&'static str], visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_struct(name: {:?}, fields: {:?})", name, fields);

    let value = self.deserialize_map(visitor)?;
    trace!("Deserializer::deserialize_struct(name: {:?}) => end", name);

    Ok(value)
  }

  fn deserialize_enum<V>(self, name: &'static str, variants: &'static [&'static str], _visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_enum(name: {:?}, variants: {:?})", name, variants);

    Err(Error::StaticMessage("enum deserialization not supported"))
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
