use std::fmt::{self, Write};
use std::marker::PhantomData;

use indexmap::IndexMap;
use serde::de::{self, Deserialize, DeserializeSeed, Error, EnumAccess, MapAccess, VariantAccess, Visitor};

use node::Node;
use node_types::StandardType;
use value::Value;

struct NodeVisitor {
  key: Option<String>,
}

impl<'de> Visitor<'de> for NodeVisitor {
  type Value = Node;

  fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str("any valid kbin node")
  }

  #[inline]
  fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where A: MapAccess<'de>
  {
    trace!("NodeVisitor::visit_map()");

    let mut attributes = None;
    let mut nodes = IndexMap::new();

    while let Some(key) = try!(map.next_key_seed(NodeSeed)) {
      debug!("NodeVisitor::visit_map() => key: {:?}", key);
      let NodeStart { key, node_type } = key;

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
        StandardType::NodeStart => {
          let value = map.next_value_seed(NodeValueSeed(key.clone()))?;
          debug!("NodeVisitor::visit_map() => value: {:?}", value);

          nodes.insert(key, value);
        },
        _ => {
          let value = map.next_value();
          debug!("NodeVisitor::visit_map() => value: {:?}", value);

          let node = Node::new(key.clone(), Some(try!(value)));
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
        },
      };
    }

    debug!("NodeVisitor::visit_map() => nodes: {:#?}", nodes);
    let node = Node {
      attributes,
      key: self.key.unwrap_or_else(|| "".to_owned()),
      children: Some(nodes),
      value: None,
    };

    //Err(A::Error::custom("still finishing implementation"))
    Ok(node)
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
