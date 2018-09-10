use std::fmt;
use std::marker::PhantomData;

use serde::de::{self, Deserialize, DeserializeSeed, Error, EnumAccess, IntoDeserializer, MapAccess, SeqAccess, VariantAccess, Visitor};
use serde::de::value::{MapDeserializer, SeqDeserializer};

use node::Node;
use node_types::StandardType;
use value::Value;

pub(crate) struct NodeVisitor {
  key: Option<String>,
}

impl<'de> NodeVisitor {
  fn map_to_node<A>(node_type: StandardType, key: &str, map: &mut A) -> Result<Node, A::Error>
    where A: MapAccess<'de>
  {
    trace!("NodeVisitor::map_to_node(node_type: {:?})", node_type);

    match node_type {
      StandardType::Attribute => Err(A::Error::custom("`Attribute` nodes must be handled elsewhere")),
      StandardType::NodeStart => {
        let value = try!(map.next_value_seed(NodeValueSeed(key.to_owned())));
        debug!("NodeVisitor::map_to_node(node_type: {:?}) => value: {:?}", node_type, value);

        Ok(value)
      },
      // Rolling up the `NodeStart` handling and other value types is not going
      // to happen as `NodeStart` nodes do not have a value
      node_type => {
        let node = try!(map.next_value_seed(NodeWithValueSeed(key.to_owned())));
        debug!("NodeVisitor::map_to_node(node_type: {:?}) => node: {:?}", node_type, node);

        Ok(node)
      },
    }
  }
}

impl<'de> Visitor<'de> for NodeVisitor {
  type Value = Node;

  fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str("any valid kbin node (for NodeVisitor)")
  }

  #[inline]
  fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where A: MapAccess<'de>
  {
    trace!("NodeVisitor::visit_map()");

    let mut node = Node::new(self.key.unwrap_or_else(|| "".to_owned()));

    while let Some(NodeStart { key, node_type }) = try!(map.next_key()) {
      debug!("NodeVisitor::visit_map() => node_type: {:?}, key: {:?}", node_type, key);

      if key == "__value" {
        trace!("NodeVisitor::visit_map() => got __value, getting node value");

        let node_value = try!(map.next_value());
        debug!("NodeVisitor::visit_map() => node value: {:?}", node_value);

        node.set_value(Some(node_value));
      } else if key == "__node_key" {
        trace!("NodeVisitor::visit_map() => got __node_key, getting node key");

        let node_key: String = try!(map.next_value());
        debug!("NodeVisitor::visit_map() => node key: {:?}", node_key);

        node.set_key(node_key);
      } else {
        match node_type {
          StandardType::Attribute => {
            let value = map.next_value();
            debug!("NodeVisitor::visit_map() => value: {:?}", value);

            if let Value::Attribute(s) = try!(value) {
              //let key = String::from(&key["attr_".len()..]);
              node.set_attr(key, s);
            } else {
              return Err(A::Error::custom("`Attribute` node must have `Value::Attribute` value"));
            }
          },
          _ => {
            let new_node = NodeVisitor::map_to_node(node_type, &key, &mut map)?;
            debug!("NodeVisitor::visit_map() => node: {:?}", node);

            node.append_child(new_node);
          },
        };
      }
    }

    Ok(node)
  }

  #[inline]
  fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where A: SeqAccess<'de>
  {
    trace!("NodeVisitor::visit_seq()");
    let key = seq.next_element()?.ok_or_else(|| A::Error::custom("first element must be `key`"))?;
    let attributes = seq.next_element()?.ok_or_else(|| A::Error::custom("second element must be `attributes`"))?;
    let children = seq.next_element()?.ok_or_else(|| A::Error::custom("third element must be `children`"))?;
    let value = seq.next_element()?.ok_or_else(|| A::Error::custom("fourth element must be `value`"))?;
    Ok(Node {
      key,
      attributes,
      children,
      value,
    })
  }
}

impl<'de> Deserialize<'de> for Node {
  #[inline]
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: de::Deserializer<'de>
  {
    deserializer.deserialize_map(NodeVisitor { key: None })
  }
}

/// A `DeserializeSeed` holder to deserialize a `Node` from `NodeDeserializer`
pub(crate) struct NodeSeed;

impl<'de> DeserializeSeed<'de> for NodeSeed {
  type Value = Node;

  #[inline]
  fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where D: de::Deserializer<'de>
  {
    // `key` will be fixed in `deserialize_seq`
    deserializer.deserialize_seq(NodeVisitor { key: None })
  }
}

struct NodeValueSeed(String);

impl<'de> DeserializeSeed<'de> for NodeValueSeed {
  type Value = Node;

  #[inline]
  fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where D: de::Deserializer<'de>
  {
    trace!("NodeValueSeed(key: {:?})::deserialize()", self.0);

    deserializer.deserialize_map(NodeVisitor { key: Some(self.0) })
  }
}

#[derive(Debug)]
pub(crate) struct NodeWithValueSeed(String);

impl<'de> DeserializeSeed<'de> for NodeWithValueSeed {
  type Value = Node;

  #[inline]
  fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where D: de::Deserializer<'de>
  {
    trace!("NodeWithValueSeed(key: {:?})::deserialize()", self.0);

    deserializer.deserialize_tuple_struct("__value", 0, NodeVisitor { key: Some(self.0) })
  }
}

pub struct NodeDeserializer<E> {
  node: Node,
  marker: PhantomData<E>,
  index: usize,
}

impl<'de, E: Error> de::Deserializer<'de> for NodeDeserializer<E> {
  type Error = E;

  #[inline]
  fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    trace!("NodeDeserializer::deserialize_any(key: {:?})", self.node.key);
    visitor.visit_seq(self)
  }

  forward_to_deserialize_any! {
    bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str
    string bytes byte_buf option unit unit_struct newtype_struct seq
    tuple tuple_struct map struct enum identifier ignored_any
  }
}

impl<'de, E: Error> SeqAccess<'de> for NodeDeserializer<E> {
  type Error = E;

  /// "Deserializes" the key, attributes as (key, string), children as
  /// (key, node), and value as itself
  fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where T: DeserializeSeed<'de>
  {
    macro_rules! make_deserializer {
      ($value:expr, $deserializer:ident) => {
        match $value.take() {
          Some(value) => {
            let deserializer = $deserializer::new(value.into_iter());
            seed.deserialize(deserializer).map(Some)
          },
          None => seed.deserialize(().into_deserializer()).map(Some),
        }
      };
    }

    trace!("--> <NodeDeserializer as SeqAccess>::next_element_seed(index: {})", self.index);
    let value = match self.index {
      0 => seed.deserialize(self.node.key.as_str().into_deserializer()).map(Some),
      1 => make_deserializer!(self.node.attributes, MapDeserializer),
      2 => make_deserializer!(self.node.children, SeqDeserializer),
      3 => match self.node.value.take() {
        Some(value) => seed.deserialize(value.into_deserializer()).map(Some),
        None => seed.deserialize(().into_deserializer()).map(Some),
      },
      _ => Ok(None),
    };
    self.index += 1;

    value
  }
}

impl<'de, E: Error> IntoDeserializer<'de, E> for Node {
  type Deserializer = NodeDeserializer<E>;

  #[inline]
  fn into_deserializer(self) -> Self::Deserializer {
    NodeDeserializer {
      node: self,
      marker: PhantomData,
      index: 0,
    }
  }
}

/// Node classifier that gets the key name and the type of the node before the
/// main `Node` object handles getting the value based on the type and the
/// attributes.
#[derive(Debug)]
pub(crate) struct NodeStart {
  pub(crate) key: String,
  pub(crate) node_type: StandardType,
}

impl<'de> Deserialize<'de> for NodeStart {
  #[inline]
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: de::Deserializer<'de>
  {
    trace!("NodeStart::deserialize()");

    struct NodeVisitor;

    impl<'de> Visitor<'de> for NodeVisitor {
      type Value = NodeStart;

      fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("enum input of a node (for NodeStart)")
      }

      #[inline]
      fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
        where A: EnumAccess<'de>
      {
        trace!("NodeVisitor::visit_enum()");
        let (id, variant): (u8, _) = data.variant()?;

        let node_type = StandardType::from_u8(id);
        debug!("NodeVisitor::visit_enum() => id: {}, node_type: {:?}", id, node_type);

        let key: String = variant.newtype_variant_seed(PhantomData)?;
        debug!("NodeVisitor::visit_enum() => key: {:?}", key);

        Ok(NodeStart { key, node_type })
      }
    }

    deserializer.deserialize_tuple_struct("__key", 0, NodeVisitor)
  }
}
