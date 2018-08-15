use indexmap::IndexMap;

use value::Value;

mod de;
mod ser;

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
  attributes: Option<IndexMap<String, Value>>,
  key: String,
  value: Value,
}

impl Node {
  pub fn new(key: String, value: Value) -> Self {
    Self {
      attributes: None,
      key,
      value,
    }
  }

  pub fn set_attr(&mut self, key: String, value: Value) -> Option<Value> {
    let attributes = self.attributes.get_or_insert_with(Default::default);
    attributes.insert(key, value)
  }
}
