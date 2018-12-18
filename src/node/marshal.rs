use std::fmt;
use std::marker::PhantomData;

use serde::de::{self, Deserialize, DeserializeSeed, IntoDeserializer, SeqAccess, Visitor};

use crate::node::Node;
use crate::node::de::NodeSeed;
use crate::node_types::StandardType;
use crate::value::Value;

#[derive(Debug)]
pub enum MarshalValue {
  Node(Node),
  Value(Value),
}

#[derive(Debug)]
pub struct Marshal {
  node_type: StandardType,
  value: MarshalValue,
}

impl MarshalValue {
  /*
  pub fn as_node(self) -> Option<Node> {
    match self {
      MarshalValue::Node(node) => Some(node),
      MarshalValue::Value(_) => None,
    }
  }
  */

  pub fn as_value(self) -> Option<Value> {
    match self {
      MarshalValue::Node(_) => None,
      MarshalValue::Value(value) => Some(value),
    }
  }
}

impl Marshal {
  pub fn with_node(node_type: StandardType, node: Node) -> Self {
    Self {
      node_type,
      value: MarshalValue::Node(node),
    }
  }

  pub fn with_value(node_type: StandardType, value: Value) -> Self {
    Self {
      node_type,
      value: MarshalValue::Value(value),
    }
  }

  pub fn into_inner(self) -> MarshalValue {
    self.value
  }
}

impl<'de> Deserialize<'de> for Marshal {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: de::Deserializer<'de>
  {
    struct MarshalVisitor;

    impl<'de> Visitor<'de> for MarshalVisitor {
      type Value = Marshal;

      fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("the components of `Marshal`")
      }

      fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where A: SeqAccess<'de>
      {
        let node_type = StandardType::from_u8(seq.next_element()?.unwrap());

        trace!("NodeMarshalVisitor::visit_seq() => node_type: {:?}", node_type);
        let value = if node_type == StandardType::NodeStart {
          let node = seq.next_element_seed(NodeSeed)?.unwrap();
          debug!("NodeMarshalVisitor::visit_seq() => node: {:?}", node);
          MarshalValue::Node(node)
        } else {
          let value = seq.next_element_seed(node_type)?.unwrap();
          debug!("NodeMarshalVisitor::visit_seq() => value: {:?}", value);
          MarshalValue::Value(value)
        };

        Ok(Marshal { node_type, value })
      }

      /// An alternate entrypoint
      fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: de::Deserializer<'de>
      {
        trace!("NodeMarshalVisitor::visit_newtype_struct()");
        Marshal::deserialize(deserializer)
      }
    }

    deserializer.deserialize_any(MarshalVisitor)
  }
}

pub struct MarshalDeserializer<E> {
  node_type: StandardType,
  value: Option<MarshalValue>,
  index: usize,
  marker: PhantomData<E>,
}

impl<'de, E> de::Deserializer<'de> for MarshalDeserializer<E>
  where E: de::Error
{
  type Error = E;

  fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    trace!("MarshalDeserializer::deserialize_any(node_type: {:?}, value: {:?})", self.node_type, self.value);
    visitor.visit_seq(self)
  }

  forward_to_deserialize_any! {
    bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str
    string bytes byte_buf option unit unit_struct newtype_struct seq
    tuple tuple_struct map struct enum identifier ignored_any
  }
}

impl<'de, E> SeqAccess<'de> for MarshalDeserializer<E>
  where E: de::Error
{
  type Error = E;

  /// Deserialize `Marshal` as a tuple
  fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where T: DeserializeSeed<'de>
  {
    trace!("<MarshalDeserializer as SeqAccess>::next_element_seed(node_type: {:?}, value: {:?}, index: {})", self.node_type, self.value, self.index);

    let result = match self.index {
      0 => seed.deserialize(self.node_type.id.into_deserializer()).map(Some),
      1 => match self.value.take() {
        Some(MarshalValue::Node(node)) => seed.deserialize(node.into_deserializer()).map(Some),
        Some(MarshalValue::Value(value)) => seed.deserialize(value.into_deserializer()).map(Some),
        None => Err(E::custom("`value` for `MarshalDeserializer` should not be `None` at `next_element_seed` at index 1")),
      },
      _ => Ok(None),
    };
    self.index += 1;

    result
  }
}

impl<'de, E> IntoDeserializer<'de, E> for Marshal
  where E: de::Error
{
  type Deserializer = MarshalDeserializer<E>;

  fn into_deserializer(self) -> Self::Deserializer {
    MarshalDeserializer {
      node_type: self.node_type,
      value: Some(self.value),
      index: 0,
      marker: PhantomData,
    }
  }
}
