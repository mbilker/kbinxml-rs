use serde::de::{self, IntoDeserializer, Visitor};

use crate::de::custom::Custom;
use crate::de::seq::Seq;
use crate::error::{Error, KbinErrorKind};
use crate::node::{Marshal, NodeCollection, NodeDefinition};
use crate::node_types::StandardType;
use crate::value::Value;

pub struct NodeDefinitionDeserializer<'a> {
  definition: &'a NodeDefinition,
}

impl<'a> NodeDefinitionDeserializer<'a> {
  pub fn new(definition: &'a NodeDefinition) -> Self {
    trace!("NodeDefinitionDeserializer::new(definition: {})", definition);

    Self { definition }
  }
}

macro_rules! auto_deserialize {
  ($($method:ident $konst:ident $visit_method:ident)*) => {
    $(
      #[inline]
      fn $method<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where V: Visitor<'de>
      {
        trace!(concat!("NodeDefinitionDeserializer::", stringify!($method), "()"));

        match self.definition.value() {
          Ok(Value::$konst(value)) => visitor.$visit_method(value),
          Ok(_) => Err(KbinErrorKind::InvalidNodeType(self.definition.node_type).into()),
          Err(e) => Err(e.into()),
        }
      }
    )*
  };
}

impl<'de, 'a> de::Deserializer<'de> for NodeDefinitionDeserializer<'a> {
  type Error = Error;

  fn is_human_readable(&self) -> bool {
    false
  }

  fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    let node_type = self.definition.node_type;
    let is_array = self.definition.is_array;
    trace!("NodeDefinitionDeserializer::deserialize_any(node_type: {:?}, is_array: {})", node_type, is_array);

    // Construct a shim `NodeCollection` for `Seq` if we are deserializing an
    // array value
    if is_array {
      let mut collection = NodeCollection::new(self.definition.clone());
      return visitor.visit_seq(Seq::new(&mut collection, true)?);
    }

    let value = match node_type {
      StandardType::NodeStart => {
        debug!("NodeDefinitionDeserializer::deserialize_any(unode_type: {:?}, is_array: {})", node_type, is_array);
        Err(KbinErrorKind::InvalidNodeType(node_type).into())
      },
      _ => {
        let value = self.definition.value()?;
        debug!("NodeDefinitionDeserializer::deserialize_any(node_type: {:?}, is_array: {}) => value: {:?}", node_type, is_array, value);
        let marshal = Marshal::with_value(node_type, value);
        visitor.visit_newtype_struct(marshal.into_deserializer())
      },
    };
    value
  }

  forward_to_deserialize_any! {
    i128 u128 char str
    seq
    map struct enum ignored_any
  }

  fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    trace!("NodeDefinitionDeserializer::deserialize_bool()");

    match self.definition.value() {
      Ok(Value::Boolean(b)) => visitor.visit_bool(b),
      Ok(_) => Err(KbinErrorKind::InvalidNodeType(self.definition.node_type).into()),
      Err(e) => Err(e.into()),
    }
  }

  auto_deserialize! {
    deserialize_i8  S8  visit_i8
    deserialize_i16 S16 visit_i16
    deserialize_i32 S32 visit_i32
    deserialize_i64 S64 visit_i64
    deserialize_u8  U8  visit_u8
    deserialize_u16 U16 visit_u16
    deserialize_u32 U32 visit_u32
    deserialize_u64 U64 visit_u64
    deserialize_f32 Float visit_f32
    deserialize_f64 Double visit_f64
    deserialize_byte_buf Binary visit_byte_buf
  }

  fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    trace!("NodeDefinitionDeserializer::deserialize_string()");

    match self.definition.value() {
      Ok(Value::String(s)) |
      Ok(Value::Attribute(s)) => visitor.visit_string(s),
      Ok(_) => Err(KbinErrorKind::InvalidNodeType(self.definition.node_type).into()),
      Err(e) => Err(e.into()),
    }
  }

  fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    trace!("NodeDefinitionDeserializer::deserialize_bytes()");

    if self.definition.node_type == StandardType::Binary {
      match self.definition.value_bytes() {
        Some(data) => visitor.visit_bytes(data),
        None => Err(KbinErrorKind::InvalidState.into()),
      }
    } else {
      Err(KbinErrorKind::InvalidNodeType(self.definition.node_type).into())
    }
  }

  fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    trace!("NodeDefinitionDeserializer::deserialize_option()");
    visitor.visit_some(self)
  }

  fn deserialize_unit<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    trace!("NodeDefinitionDeserializer::deserialize_unit()");
    Err(Error::StaticMessage("unit deserialization is not supported"))
  }

  fn deserialize_unit_struct<V>(self, name: &'static str, _visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    trace!("NodeDefinitionDeserializer::deserialize_unit_struct(name: {:?})", name);
    Err(Error::StaticMessage("unit struct deserialization is not supported"))
  }

  fn deserialize_newtype_struct<V>(self, name: &'static str, _visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    trace!("NodeDefinitionDeserializer::deserialize_newtype_struct(name: {:?})", name);
    Err(Error::StaticMessage("newtype struct deserialization is not supported"))
  }

  fn deserialize_tuple_struct<V>(self, name: &'static str, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    trace!("NodeDefinitionDeserializer::deserialize_tuple_struct(name: {:?}, len: {})", name, len);

    let node_type = self.definition.node_type;

    match name {
      "__key" => {
        let key = self.definition.key()?.ok_or(KbinErrorKind::InvalidState)?;
        let de = key.into_deserializer();
        visitor.visit_enum(Custom::new(de, node_type))
      },
      "__value" => {
        debug!("NodeDefinitionDeserializer::deserialize_tuple_struct(name: {:?}) => node_type: {:?}", name, node_type);
        self.deserialize_any(visitor)
      },
      _ => {
        Err(KbinErrorKind::InvalidNodeType(self.definition.node_type).into())
      },
    }
  }

  fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    trace!("NodeDefinitionDeserialize::deserialize_tuple(len: {})", len);

    macro_rules! tuple {
      ($($konst:ident),*) => {
        match self.definition.value() {
          $(
            Ok(value @ Value::$konst(_)) |
          )*
          Ok(value @ Value::Ip4(_)) => value.into_deserializer().deserialize_any(visitor),
          Ok(_) => Err(KbinErrorKind::InvalidNodeType(self.definition.node_type).into()),
          Err(e) => Err(e.into()),
        }
      };
    }

    tuple! {
      S8_2, U8_2, S16_2, U16_2, S32_2, U32_2, S64_2, U64_2, Float2, Double2, Boolean2,
      S8_3, U8_3, S16_3, U16_3, S32_3, U32_3, S64_3, U64_3, Float3, Double3, Boolean3,
      S8_4, U8_4, S16_4, U16_4, S32_4, U32_4, S64_4, U64_4, Float4, Double4, Boolean4,
      Vs16, Vu16,
      Vs8, Vu8, Vb
    }
  }

  fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    trace!("NodeDefinitionDeserializer::deserialize_identifier()");

    let key = self.definition.key()?.ok_or(KbinErrorKind::InvalidState)?;
    let key = match self.definition.node_type {
      StandardType::Attribute => format!("attr_{}", key),
      _ => key,
    };

    visitor.visit_string(key)
  }
}
