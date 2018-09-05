use std::iter::Iterator;

use node::NodeDefinition;
use node_types::StandardType;

/// A collection of node definitions (`NodeDefinition`)
#[derive(Debug)]
pub struct NodeCollection<'buf> {
  base: NodeDefinition<'buf>,
  attributes: Vec<NodeDefinition<'buf>>,
  children: Vec<NodeCollection<'buf>>,
}

impl<'buf> NodeCollection<'buf> {
  pub fn from_iter<I>(mut iter: I) -> Option<NodeCollection<'buf>>
    where I: Iterator<Item = NodeDefinition<'buf>>
  {
    let base = if let Some(def) = iter.next() {
      def
    } else {
      return None;
    };

    NodeCollection::with_base(base, &mut iter)
  }

  fn with_base<I>(base: NodeDefinition<'buf>, iter: &mut I) -> Option<NodeCollection<'buf>>
    where I: Iterator<Item = NodeDefinition<'buf>>
  {
    let mut attributes = Vec::new();
    let mut children = Vec::new();

    loop {
      if let Some(def) = iter.next() {
        match def.node_type {
          StandardType::Attribute => attributes.push(def),
          StandardType::NodeEnd |
          StandardType::FileEnd => break,
          _ => match NodeCollection::with_base(def, iter) {
            Some(child) => children.push(child),
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
}
