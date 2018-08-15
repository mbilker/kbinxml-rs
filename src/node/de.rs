use std::fmt::{self, Write};
use std::marker::PhantomData;

use indexmap::IndexMap;
use serde::de::{self, Deserialize, DeserializeSeed, EnumAccess, MapAccess, VariantAccess, Visitor};
use serde::de::Error as DeError;

use node::Node;
use node_types::StandardType;

impl<'de> Deserialize<'de> for Node {
  #[inline]
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: de::Deserializer<'de>
  {
    struct NodeVisitor;

    impl<'de> Visitor<'de> for NodeVisitor {
      type Value = Node;

      fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("any valid kbin node")
      }

      fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where A: MapAccess<'de>
      {
        trace!("NodeVisitor::visit_map()");

        let mut nodes: IndexMap<String, Node> = IndexMap::new();

        while let Some(key) = map.next_key_seed(NodeSeed)? {
          debug!("NodeVisitor::visit_map() => key: {:?}", key);
          let NodeStart { key, node_type } = key;

          let value = map.next_value_seed(PhantomData);
          debug!("NodeVisitor::visit_map() => value: {:?}", value);

          let node = Node::new(key.clone(), value?);
          debug!("NodeVisitor::visit_map() => node_type: {:?}, node: {:?}", node_type, node);

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
        }

        debug!("NodeVisitor::visit_map() => nodes: {:#?}", nodes);

        Err(A::Error::custom("still finishing implementation"))
        //Ok(nodes)
      }
    }

    deserializer.deserialize_map(NodeVisitor)
  }
}

/// Node classifier that gets the key name and the type of the node before the
/// main `Node` object handles getting the value based on the type and the
/// attributes.
struct NodeSeed;

#[derive(Debug)]
struct NodeStart {
  key: String,
  node_type: StandardType,
}

impl<'de> DeserializeSeed<'de> for NodeSeed {
  type Value = NodeStart;

  #[inline]
  fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where D: de::Deserializer<'de>
  {
    struct NodeVisitor;

    impl<'de> Visitor<'de> for NodeVisitor {
      type Value = NodeStart;

      fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("valid node type")
      }

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
