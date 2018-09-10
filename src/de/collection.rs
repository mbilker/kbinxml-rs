use serde::de::{self, IntoDeserializer, Visitor};

use de::custom::Custom;
use de::definition::NodeDefinitionDeserializer;
use de::node_contents::NodeContents;
use de::seq::Seq;
use de::structure::Struct;
use error::{Error, KbinErrorKind};
use node::{Marshal, NodeCollection};
use node_types::StandardType;

fn warn_attributes<'de>(value: &NodeCollection<'de>) -> Result<(), Error> {
  for attr in value.attributes() {
    let key = attr.key()?.ok_or(KbinErrorKind::InvalidState)?;
    let value = attr.value()?;
    warn!("Ignoring Attribute {} = {}", key, value);
  }

  Ok(())
}

pub struct NodeCollectionDeserializer<'a, 'de: 'a> {
  pub(crate) collection: &'a mut NodeCollection<'de>,
}

impl<'de, 'a> NodeCollectionDeserializer<'a, 'de> {
  pub fn new(collection: &'a mut NodeCollection<'de>) -> Self {
    trace!("NodeCollectionDeserializer::new() => attributes len: {}, children len: {}, base: {}",
      collection.attributes().len(),
      collection.children().len(),
      collection.base());

    Self { collection }
  }

  fn pop_node(&mut self) -> Result<NodeCollection<'de>, Error> {
    self.collection.children_mut().pop_front().ok_or(KbinErrorKind::InvalidState.into())
  }

  fn pop_node_warn(&mut self) -> Result<NodeCollection<'de>, Error> {
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

impl<'de, 'a> de::Deserializer<'de> for NodeCollectionDeserializer<'a, 'de> {
  type Error = Error;

  fn is_human_readable(&self) -> bool {
    false
  }

  fn deserialize_any<V>(mut self, visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    let collection = self.pop_node()?;

    let base = collection.base();
    let node_type = base.node_type;
    let is_array = base.is_array;

    if is_array {
      warn_attributes(&collection)?;

      trace!("NodeCollectionDeserializer::deserialize_any(node_type: {:?}, is_array: {})", node_type, is_array);
      return visitor.visit_seq(Seq::new(&mut self.collection, is_array)?);
    }

    match node_type {
      StandardType::NodeStart => {
        debug!("NodeCollectionDeserializer::deserialize_any(node_type: {:?}, is_array: {}) => deserializing node", node_type, is_array);

        let node = self.collection.as_node();
        debug!("NodeCollectionDeserializer::deserialize_any(node_type: {:?}, is_array: {}) => node: {:?}", node_type, is_array, node);

        let marshal = Marshal::with_node(StandardType::NodeStart, node?);
        visitor.visit_newtype_struct(marshal.into_deserializer())
      },
      _ => {
        warn_attributes(&collection)?;

        let value = base.value()?;
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

  fn deserialize_seq<V>(mut self, visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    let base = self.collection.base();
    let node_type = base.node_type;
    let is_array = base.is_array;
    debug!("NodeCollectionDeserializer::deserialize_seq(node_type: {:?}, is_array: {})", node_type, is_array);

    visitor.visit_seq(Seq::new(&mut self.collection, false)?)
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

    let base = self.collection.base();
    let node_type = base.node_type;

    match name {
      "__key" => {
        let key = base.key()?.ok_or(KbinErrorKind::InvalidState)?;
        let de = key.into_deserializer();
        visitor.visit_enum(Custom::new(de, node_type))
      },
      "__value" => {
        debug!("NodeCollectionDeserializer::deserialize_tuple_struct(name: {:?}) => node_type: {:?}", name, node_type);

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
