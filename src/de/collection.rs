use serde::de::{self, IntoDeserializer, Visitor};

use crate::de::custom::Custom;
use crate::de::definition::NodeDefinitionDeserializer;
use crate::de::node_contents::NodeContents;
use crate::de::seq::Seq;
use crate::de::structure::Struct;
use crate::error::{Error, KbinErrorKind};
use crate::node::{Marshal, NodeCollection};
use crate::node_types::StandardType;

fn warn_attributes(value: &NodeCollection) -> Result<(), Error> {
  for attr in value.attributes() {
    let key = attr.key()?.ok_or(KbinErrorKind::InvalidState)?;
    let value = attr.value()?;
    warn!("Ignoring Attribute {} = {}", key, value);
  }

  Ok(())
}

pub struct NodeCollectionDeserializer<'a> {
  pub(crate) collection: &'a mut NodeCollection,
}

impl<'a> NodeCollectionDeserializer<'a> {
  pub fn new(collection: &'a mut NodeCollection) -> Self {
    trace!("NodeCollectionDeserializer::new() => attributes len: {}, children len: {}, base: {}",
      collection.attributes().len(),
      collection.children().len(),
      collection.base());

    Self { collection }
  }

  fn pop_node(&mut self) -> Result<NodeCollection, Error> {
    self.collection.children_mut().pop_front().ok_or(KbinErrorKind::InvalidState.into())
  }

  fn pop_node_warn(&mut self) -> Result<NodeCollection, Error> {
    let value = self.pop_node()?;
    warn_attributes(&value)?;

    Ok(value)
  }
}

macro_rules! forward_to_definition_deserializer {
  ($($method:ident)*) => {
    $(
      #[inline]
      fn $method<V>(mut self, visitor: V) -> Result<V::Value, Self::Error>
        where V: Visitor<'de>
      {
        trace!(concat!("NodeCollectionDeserializer::", stringify!($method), "()"));
        let collection = self.pop_node()?;
        NodeDefinitionDeserializer::new(collection.base()).$method(visitor)
      }
    )*
  };
}

impl<'de, 'a> de::Deserializer<'de> for NodeCollectionDeserializer<'a> {
  type Error = Error;

  fn is_human_readable(&self) -> bool {
    false
  }

  fn deserialize_any<V>(mut self, visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    let mut collection = self.pop_node()?;
    let (node_type, is_array) = collection.base().node_type_tuple();

    if is_array {
      warn_attributes(&collection)?;

      trace!("NodeCollectionDeserializer::deserialize_any(node_type: {:?}, is_array: {})", node_type, is_array);
      return visitor.visit_seq(Seq::new(&mut collection, true)?);
    }

    match node_type {
      StandardType::NodeStart => {
        debug!("NodeCollectionDeserializer::deserialize_any(node_type: {:?}, is_array: {}) => deserializing node", node_type, is_array);

        let node = collection.as_node();
        debug!("NodeCollectionDeserializer::deserialize_any(node_type: {:?}, is_array: {}) => node: {:?}", node_type, is_array, node);

        let marshal = Marshal::with_node(StandardType::NodeStart, node?);
        visitor.visit_newtype_struct(marshal.into_deserializer())
      },
      _ => {
        warn_attributes(&collection)?;

        let value = collection.base().value()?;
        debug!("NodeCollectionDeserializer::deserialize_any(node_type: {:?}, is_array: {}) => value: {:?}", node_type, is_array, value);
        let marshal = Marshal::with_value(node_type, value);
        visitor.visit_newtype_struct(marshal.into_deserializer())
      },
    }
  }

  forward_to_deserialize_any! {
    ignored_any
  }

  forward_to_definition_deserializer! {
    deserialize_bool
    deserialize_i8
    deserialize_i16
    deserialize_i32
    deserialize_i64
    deserialize_i128
    deserialize_u8
    deserialize_u16
    deserialize_u32
    deserialize_u64
    deserialize_u128
    deserialize_f32
    deserialize_f64
    deserialize_char
    deserialize_str
    deserialize_string
    deserialize_bytes
    deserialize_byte_buf
  }

  fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    trace!("NodeCollectionDeserializer::deserialize_option()");
    visitor.visit_some(self)
  }

  fn deserialize_unit<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    trace!("NodeCollectionDeserializer::deserialize_unit()");
    Err(Error::StaticMessage("unit deserialization is not supported"))
  }

  fn deserialize_unit_struct<V>(self, name: &'static str, _visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    trace!("NodeCollectionDeserializer::deserialize_unit_struct(name: {:?})", name);
    Err(Error::StaticMessage("unit struct deserialization is not supported"))
  }

  fn deserialize_newtype_struct<V>(self, name: &'static str, _visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    trace!("NodeCollectionDeserializer::deserialize_newtype_struct(name: {:?})", name);
    Err(Error::StaticMessage("newtype struct deserialization is not supported"))
  }

  /// This will deserialize as a sequence of nodes if the first child node of
  /// `self.collection` has `is_array == false`. Else, it will pop the first
  /// child node and deserialize it as an array.
  ///
  /// This is a compromise to allow struct sequences but also allow arrays of value.
  fn deserialize_seq<V>(mut self, visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    let (node_type, is_array) = self.collection.children().front().ok_or(KbinErrorKind::InvalidState)?
      .base()
      .node_type_tuple();
    debug!("NodeCollectionDeserializer::deserialize_seq(node_type: {:?}, is_array: {})", node_type, is_array);

    if is_array {
      let mut collection = self.pop_node_warn()?;
      visitor.visit_seq(Seq::new(&mut collection, true)?)
    } else {
      visitor.visit_seq(Seq::new(&mut self.collection, false)?)
    }
  }

  fn deserialize_tuple<V>(mut self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    trace!("NodeCollectionDeserializer::deserialize_tuple(len: {})", len);

    let collection = self.pop_node_warn()?;
    NodeDefinitionDeserializer::new(collection.base()).deserialize_tuple(len, visitor)
  }

  fn deserialize_tuple_struct<V>(mut self, name: &'static str, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    trace!("NodeCollectionDeserializer::deserialize_tuple_struct(name: {:?}, len: {})", name, len);

    match name {
      "__key" => {
        let base = self.collection.base();
        let node_type = base.node_type;
        let key = base.key()?.ok_or(KbinErrorKind::InvalidState)?;
        let de = key.into_deserializer();
        visitor.visit_enum(Custom::new(de, node_type))
      },
      "__value" => {
        let mut collection = self.pop_node()?;
        visitor.visit_map(NodeContents::new(&mut collection))
      },
      _ => {
        let collection = self.pop_node_warn()?;
        NodeDefinitionDeserializer::new(collection.base()).deserialize_tuple_struct(name, len, visitor)
      },
    }
  }

  fn deserialize_map<V>(mut self, visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    trace!("NodeCollectionDeserializer::deserialize_map()");

    let mut collection = self.pop_node()?;
    visitor.visit_map(Struct::new(&mut collection))
  }

  fn deserialize_struct<V>(mut self, name: &'static str, fields: &'static [&'static str], visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    trace!("NodeCollectionDeserializer::deserialize_struct(name: {:?}, fields: {:?})", name, fields);

    let mut collection = self.pop_node()?;
    let value = visitor.visit_map(Struct::new(&mut collection))?;

    let keys: Vec<_> = self.collection.children().iter()
      .filter_map(|x| x.base().key().ok())
      .collect();
    trace!("NodeCollectionDeserializer::deserialize_struct(name: {:?}) => end, keys: {:?}", name, keys);

    Ok(value)
  }

  fn deserialize_enum<V>(self, name: &'static str, variants: &'static [&'static str], _visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    trace!("NodeCollectionDeserializer::deserialize_enum(name: {:?}, variants: {:?})", name, variants);
    Err(Error::StaticMessage("enum deserialization not supported"))
  }

  fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    trace!("NodeCollectionDeserializer::deserialize_identifier()");

    // Delegate identifier deserialization to `NodeDefinitionDeserializer`
    NodeDefinitionDeserializer::new(self.collection.base()).deserialize_identifier(visitor)
  }
}
