use std::fmt;
use std::mem;

use indexmap::IndexMap;

use value::Value;

mod collection;
pub(crate) mod de;
mod definition;
mod extra;
mod marshal;
mod ser;

pub use self::collection::NodeCollection;
pub use self::definition::{Key, NodeData, NodeDefinition};
pub use self::extra::ExtraNodes;
pub use self::marshal::{Marshal, MarshalDeserializer};

/*
match children.entry(key) {
  Entry::Occupied(mut entry) => {
    match entry.get_mut() {
      child @ Child::Single(_) => {
        let old = mem::replace(child, Child::Multiple(Vec::with_capacity(2)));
        let node = match old {
          Child::Single(node) => node,
          Child::Multiple(_) => panic!("`old` was `Child::Multiple` after checking"),
        };
        match child {
          Child::Multiple(ref mut nodes) => {
            nodes.push(node);
            nodes.push(value);
          },
          _ => panic!("Invalid result of node swap"),
        };
      },
      Child::Multiple(ref mut nodes) => {
        nodes.push(value);
      },
    };
  },
  Entry::Vacant(entry) => {
    entry.insert(Child::Single(value));
  },
};
*/

#[derive(Clone, Default, PartialEq)]
pub struct Node {
  key: String,
  attributes: Option<IndexMap<String, String>>,
  children: Option<Vec<Node>>,
  value: Option<Value>,
}

impl fmt::Debug for Node {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let mut d = f.debug_struct("Node");
    d.field("key", &self.key);

    if let Some(ref attributes) = self.attributes {
      d.field("attributes", attributes);
    }
    if let Some(ref children) = self.children {
      d.field("children", children);
    }
    if let Some(ref value) = self.value {
      d.field("value", value);
    }

    d.finish()
  }
}

impl Node {
  pub fn new(key: String) -> Self {
    Self {
      key,
      attributes: None,
      children: None,
      value: None,
    }
  }

  pub fn with_value(key: String, value: Value) -> Self {
    Self {
      key,
      attributes: None,
      children: None,
      value: Some(value),
    }
  }

  #[inline]
  pub fn key(&self) -> &str {
    &self.key
  }

  #[inline]
  pub fn attributes(&self) -> Option<&IndexMap<String, String>> {
    self.attributes.as_ref()
  }

  #[inline]
  pub fn children(&self) -> Option<&Vec<Node>> {
    self.children.as_ref()
  }

  #[inline]
  pub fn children_mut(&mut self) -> Option<&mut Vec<Node>> {
    self.children.as_mut()
  }

  #[inline]
  pub fn value(&self) -> Option<&Value> {
    self.value.as_ref()
  }

  pub fn attr(&self, key: &str) -> Option<&str> {
    self.attributes().and_then(|attributes| {
      attributes.get(key).map(String::as_str)
    })
  }

  pub fn into_key_and_value(self) -> (String, Option<Value>) {
    (self.key, self.value)
  }

  pub fn set_key(&mut self, key: String) {
    self.key = key;
  }

  pub fn set_attr<K, V>(&mut self, key: K, value: V) -> Option<String>
    where K: Into<String>,
          V: Into<String>
  {
    let attributes = self.attributes.get_or_insert_with(Default::default);
    attributes.insert(key.into(), value.into())
  }

  pub fn append_child(&mut self, value: Node) {
    let children = self.children.get_or_insert_with(Default::default);
    children.push(value);
  }

  pub fn set_value(&mut self, value: Option<Value>) -> Option<Value> {
    mem::replace(&mut self.value, value)
  }

  pub fn get_first(&self, key: &str) -> Option<&Node> {
    if let Some(ref children) = self.children {
      for node in children {
        if node.key == key {
          return Some(node);
        }
      }

      None
    } else {
      None
    }
  }

  pub fn get_first_mut(&mut self, key: &str) -> Option<&mut Node> {
    if let Some(ref mut children) = self.children {
      for node in children {
        if node.key == key {
          return Some(node);
        }
      }

      None
    } else {
      None
    }
  }
}
