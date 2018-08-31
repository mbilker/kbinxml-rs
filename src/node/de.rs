use std::fmt::{self, Write};
use std::marker::PhantomData;

use indexmap::IndexMap;
use serde::de::{self, Deserialize, DeserializeSeed, Error, EnumAccess, IntoDeserializer, MapAccess, SeqAccess, VariantAccess, Visitor};
use serde::de::value::MapDeserializer;

use node::Node;
use node_types::StandardType;
use value::Value;

pub(crate) struct NodeVisitor {
  key: Option<String>,
}

impl<'de> NodeVisitor {
  pub(crate) fn map_to_node<A>(node_type: StandardType, key: &str, map: &mut A) -> Result<Node, A::Error>
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
      // TODO: roll up `NodeStart` and everything else into a single map handler
      node_type => {
        let value = try!(map.next_value());
        debug!("NodeVisitor::map_to_node(node_type: {:?}) => value: {:?}", node_type, value);

        let node = Node::with_value(key.to_owned(), value);
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

    let mut attributes = None;
    let mut nodes = IndexMap::new();

    while let Some(NodeStart { key, node_type }) = try!(map.next_key()) {
      debug!("NodeVisitor::visit_map() => node_type: {:?}, key: {:?}", node_type, key);

      match node_type {
        StandardType::Attribute => {
          let value = map.next_value();
          debug!("NodeVisitor::visit_map() => value: {:?}", value);

          if let Value::Attribute(s) = try!(value) {
            let key = String::from(&key["attr_".len()..]);
            let attributes = attributes.get_or_insert_with(IndexMap::new);
            attributes.insert(key, s);
          } else {
            return Err(A::Error::custom("`Attribute` node must have `Value::Attribute` value"));
          }
        },
        _ => {
          let node = NodeVisitor::map_to_node(node_type, &key, &mut map)?;
          debug!("NodeVisitor::visit_map() => node: {:?}", node);

          if !nodes.contains_key(&key) {
            nodes.insert(key, node);
          } else {
            let mut new_key = format!("{}1", key);
            let mut i = 2;
            while nodes.contains_key(&new_key) {
              new_key.truncate(key.len());
              write!(new_key, "{}", i);
              i += 1;
            }
            debug!("Node::visit_map() => next open key: {:?}", new_key);
            nodes.insert(new_key, node);
          }
        },
      };
    }

    debug!("NodeVisitor::visit_map() => nodes: {:#?}", nodes);
    Ok(Node {
      key: self.key.unwrap_or_else(|| "".to_owned()),
      attributes,
      children: Some(nodes),
      value: None,
    })
  }

  #[inline]
  fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where A: SeqAccess<'de>
  {
    trace!("NodeVisitor::visit_seq()");
    let key = seq.next_element()?.unwrap();
    let attributes = seq.next_element()?.unwrap();
    let children = seq.next_element()?.unwrap();
    let value = seq.next_element()?.unwrap();
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

  /// "Deserializes" the key, attributes as (key, string), and children as
  /// (key, node)
  fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where T: DeserializeSeed<'de>
  {
    trace!("--> <NodeDeserializer as SeqAccess>::next_element_seed(index: {})", self.index);
    let value = match self.index {
      0 => seed.deserialize(self.node.key.as_str().into_deserializer()).map(Some),
      1 => match self.node.attributes.take() {
        Some(attributes) => {
          let deserializer = MapDeserializer::new(attributes.into_iter());
          seed.deserialize(deserializer).map(Some)
        },
        None => seed.deserialize(().into_deserializer()).map(Some),
      },
      2 => match self.node.children.take() {
        Some(children) => {
          let deserializer = MapDeserializer::new(children.into_iter());
          seed.deserialize(deserializer).map(Some)
        },
        None => seed.deserialize(().into_deserializer()).map(Some),
      },
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
    deserializer.deserialize_map(NodeVisitor { key: Some(self.0) })
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
        formatter.write_str("valid node type (for NodeSeed)")
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
