use std::collections::VecDeque;
use std::fmt;
use std::iter::Iterator;

use error::{KbinError, KbinErrorKind};
use node::{Node, NodeDefinition};
use node_types::StandardType;
use value::Value;

/// A collection of node definitions (`NodeDefinition`)
#[derive(Debug)]
pub struct NodeCollection {
  base: NodeDefinition,
  attributes: VecDeque<NodeDefinition>,
  children: VecDeque<NodeCollection>,
}

impl NodeCollection {
  pub fn new(base: NodeDefinition) -> Self {
    Self {
      base,
      attributes: VecDeque::with_capacity(0),
      children: VecDeque::with_capacity(0),
    }
  }

  pub fn from_iter<I>(mut iter: I) -> Option<NodeCollection>
    where I: Iterator<Item = NodeDefinition>
  {
    let base = if let Some(def) = iter.next() {
      def
    } else {
      return None;
    };

    NodeCollection::with_base(base, &mut iter)
  }

  fn with_base<I>(base: NodeDefinition, iter: &mut I) -> Option<NodeCollection>
    where I: Iterator<Item = NodeDefinition>
  {
    let mut attributes = VecDeque::new();
    let mut children = VecDeque::new();

    loop {
      if let Some(def) = iter.next() {
        match def.node_type {
          StandardType::Attribute => attributes.push_back(def),
          StandardType::NodeEnd |
          StandardType::FileEnd => break,
          _ => match NodeCollection::with_base(def, iter) {
            Some(child) => children.push_back(child),
            None => return None,
          },
        }
      } else {
        break;
      }
    }

    Some(NodeCollection {
      base,
      attributes,
      children,
    })
  }

  #[inline]
  pub fn base(&self) -> &NodeDefinition {
    &self.base
  }

  #[inline]
  pub fn attributes(&self) -> &VecDeque<NodeDefinition> {
    &self.attributes
  }

  #[inline]
  pub fn attributes_mut(&mut self) -> &mut VecDeque<NodeDefinition> {
    &mut self.attributes
  }

  #[inline]
  pub fn children(&self) -> &VecDeque<NodeCollection> {
    &self.children
  }

  #[inline]
  pub fn children_mut(&mut self) -> &mut VecDeque<NodeCollection> {
    &mut self.children
  }

  pub fn as_node(&self) -> Result<Node, KbinError> {
    let mut node = self.base.as_node()?;

    for attr in &self.attributes {
      let key = attr.key()?.ok_or(KbinErrorKind::InvalidState)?;

      if let Value::Attribute(value) = attr.value()? {
        node.set_attr(key, value);
      } else {
        return Err(KbinErrorKind::InvalidState.into());
      }
    }

    for child in &self.children {
      node.append_child(child.as_node()?);
    }

    Ok(node)
  }
}

struct DisplayDebugWrapper<'a, T: fmt::Display + 'a>(&'a T, bool);
impl<'a, T> fmt::Debug for DisplayDebugWrapper<'a, T>
  where T: fmt::Display
{
  #[inline]
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    if self.1 {
      write!(f, "{:#}", self.0)
    } else {
      write!(f, "{}", self.0)
    }
  }
}

struct VecDisplayDebugWrapper<'a, T: fmt::Display + 'a>(&'a VecDeque<T>, bool);
impl<'a, T> fmt::Debug for VecDisplayDebugWrapper<'a, T>
  where T: fmt::Display
{
  #[inline]
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let mut l = f.debug_list();
    for entry in self.0 {
      l.entry(&DisplayDebugWrapper(&entry, self.1));
    }
    l.finish()
  }
}

impl fmt::Display for NodeCollection {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let mut d = f.debug_struct("NodeCollection");

    d.field("base", &DisplayDebugWrapper(&self.base, false));
    d.field("attributes", &VecDisplayDebugWrapper(&self.attributes, false));
    d.field("children", &VecDisplayDebugWrapper(&self.children, true));

    d.finish()
  }
}
