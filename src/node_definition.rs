use std::io::Cursor;

use error::KbinError;
use node::Node;
use node_types::StandardType;
use sixbit::{Sixbit, SixbitSize};

#[derive(Debug)]
pub enum Key<'buf> {
  Some {
    size: SixbitSize,
    data: &'buf [u8],
  },
  None,
}

#[derive(Debug)]
pub struct NodeDefinition<'buf> {
  pub node_type: StandardType,
  pub is_array: bool,

  pub key: Key<'buf>,

  pub value_data: Option<&'buf [u8]>,
}

impl<'buf> NodeDefinition<'buf> {
  /*
  pub fn new(
    node_type: (StandardType, bool),
    value_data: Option<&'buf [u8]>,
  ) -> Self {
    Self::with_key(node_type, Key::None, value_data)
  }
  */

  pub fn with_key(
    node_type: (StandardType, bool),
    key: Key<'buf>,
    value_data: Option<&'buf [u8]>,
  ) -> Self {
    let (node_type, is_array) = node_type;

    Self {
      node_type,
      is_array,
      key,
      value_data,
    }
  }

  pub fn key(&self) -> Result<Option<String>, KbinError> {
    match self.key {
      Key::Some { ref size, ref data } => {
        let mut data = Cursor::new(data);
        Ok(Some(Sixbit::unpack(&mut data, *size)?))
      },
      Key::None => Ok(None),
    }
  }

  pub fn into_node(self) -> Result<Node, KbinError> {
    let key = self.key()?.unwrap_or_else(|| String::new());

    if let Some(_value_data) = self.value_data {
      //Ok(Node::with_value(key, value))
      unimplemented!();
    } else {
      Ok(Node::new(key))
    }
  }
}
