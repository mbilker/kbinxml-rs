use std::result::Result as StdResult;

use byteorder::{BigEndian, ByteOrder, ReadBytesExt};
use failure::ResultExt;
use serde::de::Visitor;
use serde::de::Deserializer as SerdeDeserializer;

use de::{Custom, Deserializer};
use error::{Error, KbinErrorKind};
use node_types::StandardType;

pub type Result<T> = StdResult<T, Error>;

pub struct TypeDeserializer<'a, 'de: 'a> {
  parent: &'a mut Deserializer<'de>,
  node_type: StandardType,
  is_array: bool,
}

impl<'de, 'a> TypeDeserializer<'a, 'de> {
  pub fn new(parent: &'a mut Deserializer<'de>, node_type: StandardType, is_array: bool) -> Self {
    Self {
      parent,
      node_type,
      is_array,
    }
  }
}

macro_rules! de_type {
  (byte; $method:ident, $visit_method:ident, $standard_type:ident $($cast:tt)*) => {
    fn $method<V>(self, visitor: V) -> Result<V::Value>
      where V: Visitor<'de>
    {
      let value = if self.is_array {
        self.parent.reader.read_u8().context(KbinErrorKind::DataRead(1))? $($cast)*
      } else {
        self.parent.reader.data_buf.get_aligned(*StandardType::$standard_type)?[0] $($cast)*
      };
      trace!(concat!("TypeDeserializer::", stringify!($method), "() => value: {:?}"), value);

      visitor.$visit_method(value)
    }
  };
  (large; $method:ident, $visit_method:ident, $read_method:ident, $standard_type:ident) => {
    fn $method<V>(self, visitor: V) -> Result<V::Value>
      where V: Visitor<'de>
    {
      let value = if self.is_array {
        self.parent.reader.data_buf.$read_method::<BigEndian>().context(KbinErrorKind::DataRead(StandardType::$standard_type.size as usize))?
      } else {
        let value = self.parent.reader.data_buf.get_aligned(*StandardType::$standard_type)?;
        BigEndian::$read_method(&value)
      };
      trace!(concat!("TypeDeserializer::", stringify!($method), "() => value: {:?}"), value);

      visitor.$visit_method(value)
    }
  }
}

macro_rules! implement_type {
  ($method:ident) => {
    fn $method<V>(self, _visitor: V) -> Result<V::Value>
      where V: Visitor<'de>
    {
      trace!("TypeDeserializer::{}()", stringify!($method));
      unimplemented!();
    }
  }
}

impl<'de, 'a> SerdeDeserializer<'de> for TypeDeserializer<'a, 'de> {
  type Error = Error;

  fn is_human_readable(&self) -> bool {
    false
  }

  fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("TypeDeserializer::deserialize_any(node_type: {:?}, is_array: {})", self.node_type, self.is_array);

    // Handle arrays if we are not in array reading mode
    /*
    if is_array {
      // `Ip4` handling handled by `deserialize_seq`
      match self.read_mode {
        ReadMode::Array => {},
        _ => return self.deserialize_seq(visitor),
      };
    }
    */

    let value = match self.node_type {
      StandardType::Attribute => self.deserialize_string(visitor),
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
      StandardType::Boolean => self.deserialize_bool(visitor),
      StandardType::NodeStart => unimplemented!(),
      StandardType::NodeEnd => unimplemented!(),
      _ => visitor.visit_enum(Custom::new(self.parent, self.node_type)),
    };
    value
  }

  fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("TypeDeserializer::deserialize_bool()");

    let value = self.parent.reader.data_buf.get_aligned(*StandardType::Boolean)?[0];
    trace!("TypeDeserializer::deserialize_bool() => value: {:?}", value);

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
    trace!("TypeDeserializer::deserialize_string()");
    visitor.visit_string(self.parent.reader.read_string()?)
    /*
    match self.read_mode {
      ReadMode::Key => self.deserialize_identifier(visitor),
      _ => visitor.visit_string(self.reader.read_string()?),
    }
    */
  }

  fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("TypeDeserializer::deserialize_bytes()");

    visitor.visit_bytes(self.parent.reader.read_bytes()?)
  }

  fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("TypeDeserializer::deserialize_byte_buf()");

    visitor.visit_byte_buf(self.parent.reader.read_bytes()?.to_vec())
  }

  fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("TypeDeserializer::deserialize_option()");

    // A `None` value will not occur because it will not be present in the input data
    visitor.visit_some(self)
  }

  implement_type!(deserialize_unit);

  fn deserialize_unit_struct<V>(self, name: &'static str, _visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("TypeDeserializer::deserialize_unit_struct(name: {:?})", name);
    unimplemented!();
  }

  fn deserialize_newtype_struct<V>(self, name: &'static str, _visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("TypeDeserializer::deserialize_newtype_struct(name: {:?})", name);
    unimplemented!();
  }

  fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("TypeDeserializer::deserialize_seq()");
    unimplemented!();
  }

  fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("TypeDeserializer::deserialize_tuple(len: {})", len);
    unimplemented!();

    /*
    let (node_type, is_array) = *self.node_stack_last()?;
    debug!("Deserializer::deserialize_tuple(len: {}) => node_type: {:?}, is_array: {}", len, node_type, is_array);

    // Handle case where kbin has an array but the Serde output is using a
    // tuple
    if is_array && self.read_mode == ReadMode::Single {
      return self.deserialize_seq(visitor);
    }

    //self.deserialize_seq(visitor)
    let old_read_mode = self.set_read_mode(ReadMode::Array);
    let value = visitor.visit_seq(Seq::new(self, Some(len))?)?;
    self.read_mode = old_read_mode;

    // Only realign after the outermost array finishes reading
    if self.read_mode == ReadMode::Single {
      self.reader.data_buf.realign_reads(None)?;
    }

    Ok(value)
    */
  }

  fn deserialize_tuple_struct<V>(self, name: &'static str, len: usize, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("TypeDeserializer::deserialize_tuple_struct(name: {:?}, len: {})", name, len);
    unimplemented!();

    /*
    if name == "__key" {
      let (node_type, _is_array) = self.reader.last_node_type().ok_or(KbinErrorKind::InvalidState)?;
      visitor.visit_enum(Custom::new(self, node_type))
    } else {
      let old_read_mode = self.set_read_mode(ReadMode::Array);
      let value = visitor.visit_seq(Seq::new(self, Some(len))?)?;
      self.read_mode = old_read_mode;
      self.reader.data_buf.realign_reads(None)?;

      Ok(value)
    }
    */
  }

  fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("TypeDeserializer::deserialize_map()");
    unimplemented!();

    // The `NodeStart` event is consumed by `deserialize_identifier` when
    // reading the parent struct, don't consume the next event.
    /*
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
    */
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
    unimplemented!();
  }

  fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("TypeDeserializer::deserialize_identifier()");
    visitor.visit_string(self.parent.reader.read_node_identifier()?)

    /*
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
    */
  }

  fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_ignored_any()");
    self.deserialize_any(visitor)
  }
}
