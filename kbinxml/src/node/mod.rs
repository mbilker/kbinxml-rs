use std::fmt;
use std::mem;

use indexmap::IndexMap;

use crate::value::Value;

mod collection;
mod definition;

pub use self::collection::NodeCollection;
pub use self::definition::{Key, NodeData, NodeDefinition};

// The attributes argument is very hard to generalize
fn convert_attributes(attrs: &[(&str, &str)]) -> IndexMap<String, String> {
    attrs
        .iter()
        .map(|(key, value)| (String::from(*key), String::from(*value)))
        .collect()
}

fn parse_index(s: &str) -> Option<usize> {
    if s.starts_with('+') || (s.starts_with('0') && s.len() != 1) {
        return None;
    }
    s.parse().ok()
}

#[derive(Clone, Default, PartialEq)]
pub struct Node {
    key: String,
    attributes: IndexMap<String, String>,
    children: Vec<Node>,
    value: Option<Value>,
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut d = f.debug_struct("Node");

        d.field("key", &self.key);
        d.field("attributes", &self.attributes);
        d.field("children", &self.children);

        if let Some(ref value) = self.value {
            d.field("value", value);
        }

        d.finish()
    }
}

impl Node {
    pub fn new<K>(key: K) -> Self
    where
        K: Into<String>,
    {
        Self {
            key: key.into(),
            attributes: IndexMap::new(),
            children: Vec::new(),
            value: None,
        }
    }

    pub fn with_attrs<K>(key: K, attrs: &[(&str, &str)]) -> Self
    where
        K: Into<String>,
    {
        Self {
            key: key.into(),
            attributes: convert_attributes(attrs),
            children: Vec::new(),
            value: None,
        }
    }

    pub fn with_value<K>(key: K, value: Value) -> Self
    where
        K: Into<String>,
    {
        Self {
            key: key.into(),
            attributes: IndexMap::new(),
            children: Vec::new(),
            value: Some(value),
        }
    }

    pub fn with_nodes<K, N>(key: K, nodes: N) -> Self
    where
        K: Into<String>,
        N: Into<Vec<Node>>,
    {
        Self {
            key: key.into(),
            attributes: IndexMap::new(),
            children: nodes.into(),
            value: None,
        }
    }

    pub fn with<K, N>(key: K, attrs: &[(&str, &str)], nodes: N) -> Self
    where
        K: Into<String>,
        N: Into<Vec<Node>>,
    {
        Self {
            key: key.into(),
            attributes: convert_attributes(attrs),
            children: nodes.into(),
            value: None,
        }
    }

    pub fn with_attrs_value<K>(key: K, attrs: &[(&str, &str)], value: Value) -> Self
    where
        K: Into<String>,
    {
        Self {
            key: key.into(),
            attributes: convert_attributes(attrs),
            children: Vec::new(),
            value: Some(value),
        }
    }

    #[inline]
    pub fn key(&self) -> &str {
        &self.key
    }

    #[inline]
    pub fn attributes(&self) -> &IndexMap<String, String> {
        &self.attributes
    }

    #[inline]
    pub fn attributes_mut(&mut self) -> &mut IndexMap<String, String> {
        &mut self.attributes
    }

    #[inline]
    pub fn children(&self) -> &[Node] {
        &self.children
    }

    #[inline]
    pub fn children_mut(&mut self) -> &mut Vec<Node> {
        &mut self.children
    }

    #[inline]
    pub fn value(&self) -> Option<&Value> {
        self.value.as_ref()
    }

    #[inline]
    pub fn value_mut(&mut self) -> Option<&mut Value> {
        self.value.as_mut()
    }

    pub fn into_key_value(self) -> (String, Option<Value>) {
        (self.key, self.value)
    }

    pub fn set_key<K>(&mut self, key: K)
    where
        K: Into<String>,
    {
        self.key = key.into();
    }

    pub fn set_attr<K, V>(&mut self, key: K, value: V) -> Option<String>
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.attributes.insert(key.into(), value.into())
    }

    pub fn sort_attrs(&mut self) {
        self.attributes.sort_keys();
    }

    pub fn append_child(&mut self, value: Node) {
        self.children.push(value);
    }

    pub fn set_value(&mut self, value: Option<Value>) -> Option<Value> {
        mem::replace(&mut self.value, value)
    }

    pub fn has(&self, key: &str) -> bool {
        self.children.iter().any(|node| node.key == key)
    }

    pub fn get_child(&self, key: &str) -> Option<&Node> {
        self.children.iter().find(|node| node.key == key)
    }

    pub fn get_child_mut(&mut self, key: &str) -> Option<&mut Node> {
        self.children.iter_mut().find(|node| node.key == key)
    }

    pub fn remove_child(&mut self, key: &str) -> Option<Node> {
        if let Some(index) = self.children.iter().position(|node| node.key == key) {
            Some(self.children.remove(index))
        } else {
            None
        }
    }

    pub fn pointer<'a>(&'a self, pointer: &[&str]) -> Option<&'a Node> {
        if pointer.is_empty() {
            return Some(self);
        }

        let mut target = self;

        for token in pointer {
            let target_opt = if let Some(index) = parse_index(token) {
                target.children.get(index)
            } else {
                target.children.iter().find(|child| child.key == *token)
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
            let target_opt = if let Some(index) = parse_index(token) {
                target.children.get_mut(index)
            } else {
                target.children.iter_mut().find(|child| child.key == *token)
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
