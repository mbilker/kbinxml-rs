use std::fmt;

use serde::de::{self, Deserialize, Error, MapAccess, Visitor};

use node::ExtraNodes;
use node::de::{NodeSeed, NodeStart, NodeVisitor};
use node_types::StandardType;
use value::Value;

impl<'de> Deserialize<'de> for ExtraNodes {
  #[inline]
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: de::Deserializer<'de>
  {
    struct ExtraNodesVisitor;

    impl<'de> Visitor<'de> for ExtraNodesVisitor {
      type Value = ExtraNodes;

      fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("any valid map of kbin nodes")
      }

      #[inline]
      fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where A: MapAccess<'de>
      {
        trace!("ExtraNodesVisitor::visit_map()");

        let mut extra = ExtraNodes::new();

        while let Some(NodeStart { key, node_type }) = try!(map.next_key_seed(NodeSeed)) {
          debug!("ExtraNodesVisitor::visit_map() => key: {:?}, node_type: {:?}", key, node_type);

          match node_type {
            StandardType::Attribute => {
              let value = try!(map.next_value());
              debug!("ExtraNodesVisitor::visit_map() => value: {:?}", value);

              if let Value::Attribute(s) = value {
                let key = String::from(&key["attr_".len()..]);
                extra.attributes.insert(key, s);
              } else {
                return Err(A::Error::custom("`Attribute` node must have `Value::Attribute` value"));
              }
            },
            _ => {
              let node = NodeVisitor::map_to_node(node_type, &key, &mut map)?;
              debug!("ExtraNodesVisitor::visit_map() => node: {:?}", node);
            },
          };
        }

        Ok(extra)
      }
    }

    deserializer.deserialize_map(ExtraNodesVisitor)
  }
}
