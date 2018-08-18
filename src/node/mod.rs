use indexmap::IndexMap;

use value::Value;

mod de;
mod ser;

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
  key: String,
  attributes: Option<IndexMap<String, String>>,
  children: Option<IndexMap<String, Node>>,
  value: Option<Value>,
}

impl Node {
  pub fn new(key: String, value: Option<Value>) -> Self {
    Self {
      key,
      attributes: None,
      children: None,
      value,
    }
  }

  #[inline]
  pub fn children(&self) -> Option<&IndexMap<String, Node>> {
    self.children.as_ref()
  }

  pub fn set_attr(&mut self, key: String, value: String) -> Option<String> {
    let attributes = self.attributes.get_or_insert_with(Default::default);
    attributes.insert(key, value)
  }

  pub fn insert(&mut self, key: String, value: Node) -> Option<Node> {
    let children = self.children.get_or_insert_with(Default::default);
    children.insert(key, value)
  }
}
