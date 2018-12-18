use std::mem;

use indexmap::IndexMap;

use crate::node::Node;

mod de;
mod ser;

/// Container for extra `Node` and `Attribute` objects that are not part of a
/// parent object
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExtraNodes {
  parent_key: String,
  attributes: IndexMap<String, String>,
  nodes: IndexMap<String, Node>,
}

impl ExtraNodes {
  pub fn new() -> Self {
    Self {
      parent_key: String::with_capacity(0),
      attributes: IndexMap::new(),
      nodes: IndexMap::new(),
    }
  }

  #[inline]
  pub fn parent_key(&self) -> &str {
    &self.parent_key
  }

  #[inline]
  pub fn attributes(&self) -> &IndexMap<String, String> {
    &self.attributes
  }

  #[inline]
  pub fn nodes(&self) -> &IndexMap<String, Node> {
    &self.nodes
  }

  pub fn set_parent_key(&mut self, key: String) -> String {
    mem::replace(&mut self.parent_key, key)
  }

  pub fn set_attr(&mut self, key: String, value: String) -> Option<String> {
    self.attributes.insert(key, value)
  }

  pub fn insert(&mut self, key: String, value: Node) -> Option<Node> {
    self.nodes.insert(key, value)
  }
}
