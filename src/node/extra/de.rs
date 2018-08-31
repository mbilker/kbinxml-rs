use std::fmt;

use serde::de::{self, Deserialize, Error, MapAccess, Visitor};

use node::Node;
use node::extra::ExtraNodes;
use node::marshal::{Marshal, MarshalValue};
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
        formatter.write_str("any valid map of kbin nodes (for ExtraNodes)")
      }

      #[inline]
      fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where A: MapAccess<'de>
      {
        trace!("ExtraNodesVisitor::visit_map()");

        let mut extra = ExtraNodes::new();

        loop {
          let key = match map.next_key::<String>() {
            Ok(Some(s)) => s,
            Ok(None) => break,
            Err(e) => {
              error!("ExtraNodesVisitor::visit_map() => error: {:?}", e);
              return Err(e);
            },
          };
          debug!("ExtraNodesVisitor::visit_map() => key: {:?}", key);

          let marshal: Marshal = try!(map.next_value());
          debug!("ExtraNodesVisitor::visit_map() => marshal: {:?}", marshal);

          let value = marshal.into_inner();

          if key.starts_with("attr_") {
            let key = String::from(&key["attr_".len()..]);
            debug!("ExtraNodesVisitor::visit_map() => found attribute, key: {:?}, value: {:?}", key, value);

            if let Some(value) = value.as_value() {
              if let Value::Attribute(s) = value {
                extra.set_attr(key, s);
              } else {
                return Err(A::Error::custom("`Attribute` node must have `Value::Attribute` value"));
              }
            } else {
              return Err(A::Error::custom("`Marshal` must contain `Value` for attribute"));
            }
          } else {
            /*
            let node = NodeVisitor::map_to_node(node_type, &key, &mut map)?;
            debug!("ExtraNodesVisitor::visit_map() => node: {:?}", node);
            */
            match value {
              MarshalValue::Value(value) => extra.insert(key.clone(), Node::with_value(key, value)),
              MarshalValue::Node(mut node) => {
                node.key = key.clone();
                extra.insert(key, node)
              },
            };
          }
        }

        Ok(extra)
      }
    }

    deserializer.deserialize_map(ExtraNodesVisitor)
  }
}
