use std::collections::VecDeque;
use std::fmt;
use std::iter::{FromIterator, Iterator};

use crate::error::KbinError;
use crate::node::{Node, NodeDefinition};
use crate::node_types::StandardType;
use crate::value::Value;

fn parse_index(s: &str) -> Option<usize> {
    if s.starts_with('+') || (s.starts_with('0') && s.len() != 1) {
        return None;
    }
    s.parse().ok()
}

/// A collection of node definitions (`NodeDefinition`)
#[derive(Clone, Debug, Eq, PartialEq)]
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

    pub fn with_attributes(base: NodeDefinition, attributes: VecDeque<NodeDefinition>) -> Self {
        Self {
            base,
            attributes,
            children: VecDeque::with_capacity(0),
        }
    }

    fn from_iter_base<I>(base: NodeDefinition, iter: &mut I) -> Option<NodeCollection>
    where
        I: Iterator<Item = NodeDefinition>,
    {
        let mut attributes = VecDeque::new();
        let mut children = VecDeque::new();

        while let Some(def) = iter.next() {
            match def.node_type {
                StandardType::Attribute => attributes.push_back(def),
                StandardType::NodeEnd | StandardType::FileEnd => break,
                _ => match NodeCollection::from_iter_base(def, iter) {
                    Some(child) => children.push_back(child),
                    None => return None,
                },
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
    pub fn base_mut(&mut self) -> &mut NodeDefinition {
        &mut self.base
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
            let key = attr.key()?.ok_or(KbinError::InvalidState)?;

            if let Value::Attribute(value) = attr.value()? {
                node.set_attr(key, value);
            } else {
                return Err(KbinError::InvalidState);
            }
        }

        for child in &self.children {
            node.append_child(child.as_node()?);
        }

        Ok(node)
    }

    pub fn pointer<'a>(&'a self, pointer: &[&str]) -> Option<&'a NodeCollection> {
        if pointer.is_empty() {
            return Some(self);
        }
        let mut target = self;

        for token in pointer {
            let target_opt = if let Some(index) = parse_index(token) {
                target.children().get(index)
            } else {
                target.children().iter().find(|child| {
                    child
                        .base()
                        .key()
                        .ok()
                        .and_then(|x| x)
                        .expect("key not parseable") ==
                        *token
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

impl FromIterator<NodeDefinition> for Option<NodeCollection> {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = NodeDefinition>,
    {
        let mut iter = iter.into_iter();
        let base = iter.next()?;

        NodeCollection::from_iter_base(base, &mut iter)
    }
}

struct DisplayDebugWrapper<'a, T: fmt::Display + 'a>(&'a T, bool);
impl<'a, T> fmt::Debug for DisplayDebugWrapper<'a, T>
where
    T: fmt::Display,
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
where
    T: fmt::Display,
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
        d.field(
            "attributes",
            &VecDisplayDebugWrapper(&self.attributes, false),
        );
        d.field("children", &VecDisplayDebugWrapper(&self.children, true));

        d.finish()
    }
}
