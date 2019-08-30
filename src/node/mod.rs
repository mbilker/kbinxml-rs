use std::fmt;
use std::iter::IntoIterator;
use std::mem;

use indexmap::IndexMap;

use crate::value::Value;

mod collection;
mod definition;

pub use self::collection::NodeCollection;
pub use self::definition::{Key, NodeData, NodeDefinition};

// The attributes argument is very hard to generalize
fn convert_attributes(attrs: &[(&str, &str)]) -> IndexMap<String, String> {
  attrs.iter()
    .map(|(key, value)| (String::from(*key), String::from(*value)))
    .collect()
}

fn parse_index(s: &str) -> Option<usize> {
  if s.starts_with('+') || (s.starts_with('0') && s.len() != 1) {
    return None;
  }
  s.parse().ok()
}

pub struct OptionIterator<T: IntoIterator> {
  inner: Option<T::IntoIter>,
}

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
  pub fn new<K>(key: K) -> Self
    where K: Into<String>
  {
    Self {
      key: key.into(),
      attributes: None,
      children: None,
      value: None,
    }
  }

  pub fn with_attrs<K>(key: K, attrs: &[(&str, &str)]) -> Self
    where K: Into<String>
  {
    Self {
      key: key.into(),
      attributes: Some(convert_attributes(attrs)),
      children: None,
      value: None,
    }
  }

  pub fn with_value<K>(key: K, value: Value) -> Self
    where K: Into<String>
  {
    Self {
      key: key.into(),
      attributes: None,
      children: None,
      value: Some(value),
    }
  }

  pub fn with_nodes<K, N>(key: K, nodes: N) -> Self
    where K: Into<String>,
          N: Into<Vec<Node>>
  {
    Self {
      key: key.into(),
      attributes: None,
      children: Some(nodes.into()),
      value: None,
    }
  }

  pub fn with<K, N>(key: K, attrs: &[(&str, &str)], nodes: N) -> Self
    where K: Into<String>,
          N: Into<Vec<Node>>
  {
    Self {
      key: key.into(),
      attributes: Some(convert_attributes(attrs)),
      children: Some(nodes.into()),
      value: None,
    }
  }

  pub fn with_attrs_value<K>(key: K, attrs: &[(&str, &str)], value: Value) -> Self
    where K: Into<String>
  {
    Self {
      key: key.into(),
      attributes: Some(convert_attributes(attrs)),
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
  pub fn attributes_mut(&mut self) -> Option<&mut IndexMap<String, String>> {
    self.attributes.as_mut()
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

  #[inline]
  pub fn value_mut(&mut self) -> Option<&mut Value> {
    self.value.as_mut()
  }

  #[inline]
  pub fn children_iter(&self) -> OptionIterator<&Vec<Node>> {
    OptionIterator::new(self.children())
  }

  #[inline]
  pub fn children_iter_mut(&mut self) -> OptionIterator<&mut Vec<Node>> {
    OptionIterator::new(self.children_mut())
  }

  pub fn attr(&self, key: &str) -> Option<&str> {
    self.attributes().and_then(|attributes| {
      attributes.get(key).map(String::as_str)
    })
  }

  pub fn attr_mut(&mut self, key: &str) -> Option<&mut String> {
    self.attributes_mut().and_then(|attributes| {
      attributes.get_mut(key)
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

  pub fn remove_attr(&mut self, key: &str) -> Option<String> {
    self.attributes.as_mut().and_then(|attributes| attributes.remove(key))
  }

  pub fn sort_attrs(&mut self) {
    if let Some(ref mut attributes) = self.attributes {
      attributes.sort_keys();
    }
  }

  pub fn append_child(&mut self, value: Node) {
    let children = self.children.get_or_insert_with(Default::default);
    children.push(value);
  }

  pub fn set_value(&mut self, value: Option<Value>) -> Option<Value> {
    mem::replace(&mut self.value, value)
  }

  pub fn has(&self, key: &str) -> bool {
    if let Some(ref children) = self.children {
      for node in children {
        if node.key == key {
          return true;
        }
      }
    }

    false
  }

  pub fn get_child(&self, key: &str) -> Option<&Node> {
    if let Some(ref children) = self.children {
      for node in children {
        if node.key == key {
          return Some(node);
        }
      }
    }

    None
  }

  pub fn get_child_mut(&mut self, key: &str) -> Option<&mut Node> {
    if let Some(ref mut children) = self.children {
      for node in children {
        if node.key == key {
          return Some(node);
        }
      }
    }

    None
  }

  pub fn remove_child(&mut self, key: &str) -> Option<Node> {
    if let Some(ref mut children) = self.children {
      let index = children.iter()
        .enumerate()
        .find(|(_, child)| child.key() == key)
        .map(|(index, _)| index);

      if let Some(index) = index {
        return Some(children.remove(index));
      }
    }

    None
  }

  pub fn remove_child_at(&mut self, index: usize) -> Option<Node> {
    self.children.as_mut().map(|children| children.remove(index))
  }

  pub fn pointer<'a>(&'a self, pointer: &[&str]) -> Option<&'a Node> {
    if pointer.is_empty() {
      return Some(self);
    }
    let mut target = self;

    for token in pointer {
      let children = match target.children {
        Some(ref v) => v,
        None => return None,
      };

      let target_opt = if let Some(index) = parse_index(token) {
        children.get(index)
      } else {
        children.iter().find(|ref child| {
          child.key() == *token
        })
      };

      if let Some(t) = target_opt {
        target = t;
      } else {
        return None;
      }
    }
    Some(target)
  }

  pub fn pointer_mut<'a>(&'a mut self, pointer: &[&str]) -> Option<&'a mut Node> {
    if pointer.is_empty() {
      return Some(self);
    }
    let mut target = self;

    for token in pointer {
      let children = match target.children {
        Some(ref mut v) => v,
        None => return None,
      };

      let target_opt = if let Some(index) = parse_index(token) {
        children.get_mut(index)
      } else {
        children.iter_mut().find(|ref child| {
          child.key() == *token
        })
      };

      if let Some(t) = target_opt {
        target = t;
      } else {
        return None;
      }
    }
    Some(target)
  }
}

impl<T> OptionIterator<T>
  where T: IntoIterator
{
  pub fn new(inner: Option<T>) -> Self {
    OptionIterator {
      inner: inner.map(|inner| inner.into_iter()),
    }
  }
}

impl<T> Iterator for OptionIterator<T>
  where T: IntoIterator
{
  type Item = T::Item;

  fn next(&mut self) -> Option<Self::Item> {
    match self.inner {
      Some(ref mut inner) => inner.next(),
      None => None,
    }
  }
}
