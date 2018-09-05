use std::io::Cursor;

use byte_buffer::strip_trailing_null_bytes;
use encoding_type::EncodingType;
use error::{KbinError, KbinErrorKind};
use node::Node;
use node_types::StandardType;
use sixbit::{Sixbit, SixbitSize};
use value::Value;

#[derive(Clone, Copy, Debug)]
pub enum Key<'buf> {
  Compressed {
    size: SixbitSize,
    data: &'buf [u8],
  },
  Uncompressed {
    encoding: EncodingType,
    data: &'buf [u8],
  },
}

#[derive(Clone, Copy, Debug)]
pub enum NodeData<'buf> {
  Some {
    key: Key<'buf>,
    value_data: &'buf [u8],
  },
  None,
}

#[derive(Clone, Copy, Debug)]
pub struct NodeDefinition<'buf> {
  encoding: EncodingType,
  pub node_type: StandardType,
  pub is_array: bool,

  data: NodeData<'buf>,
}

impl<'buf> Key<'buf> {
  fn to_string(&self) -> Result<String, KbinError> {
    match self {
      Key::Compressed { ref size, ref data } => {
        let mut data = Cursor::new(data);
        Ok(Sixbit::unpack(&mut data, *size)?)
      },
      Key::Uncompressed { encoding, ref data } => {
        Ok(encoding.decode_bytes(data)?)
      },
    }
  }
}

impl<'buf> NodeDefinition<'buf> {
  pub fn new(encoding: EncodingType, node_type: (StandardType, bool)) -> Self {
    let (node_type, is_array) = node_type;

    Self {
      encoding,
      node_type,
      is_array,
      data: NodeData::None,
    }
  }

  pub fn with_data(encoding: EncodingType, node_type: (StandardType, bool), data: NodeData<'buf>) -> Self {
    let (node_type, is_array) = node_type;

    Self {
      encoding,
      node_type,
      is_array,
      data,
    }
  }

  pub fn key(&self) -> Result<Option<String>, KbinError> {
    match self.data {
      NodeData::Some { ref key, .. } => key.to_string().map(Some),
      NodeData::None => Ok(None),
    }
  }

  pub fn value(&self) -> Result<Value, KbinError> {
    match (self.node_type, self.data) {
      (StandardType::Attribute, NodeData::Some { ref value_data, .. }) => {
        let data = strip_trailing_null_bytes(value_data);
        let value = self.encoding.decode_bytes(data)?;
        Ok(Value::Attribute(value))
      },
      (StandardType::String, NodeData::Some { ref value_data, .. }) => {
        let data = strip_trailing_null_bytes(value_data);
        let value = self.encoding.decode_bytes(data)?;
        Ok(Value::String(value))
      },
      (node_type, NodeData::Some { ref value_data, .. }) => {
        let value = Value::from_standard_type(node_type, self.is_array, value_data)?;
        debug!("value: {:?}", value);
        match value {
          Some(value) => Ok(value),
          None => Err(KbinErrorKind::InvalidNodeType(node_type).into()),
        }
      },
      (node_type, NodeData::None) => {
        Err(KbinErrorKind::InvalidNodeType(node_type).into())
      },
    }
  }

  pub fn as_node(&self) -> Result<Node, KbinError> {
    trace!("parsing definition: {:?}", self);
    match (self.node_type, self.data) {
      (StandardType::NodeEnd, _) |
      (StandardType::FileEnd, _) => {
        Err(KbinErrorKind::InvalidNodeType(self.node_type).into())
      },
      (StandardType::NodeStart, NodeData::Some { key, .. }) => {
        let key = key.to_string()?;
        Ok(Node::new(key))
      },
      (_, NodeData::Some { key, .. }) => {
        let key = key.to_string()?;
        let value = self.value()?;
        debug!("value: {:?}", value);
        Ok(Node::with_value(key, value))
      },
      (node_type, NodeData::None) => {
        Err(KbinErrorKind::InvalidNodeType(node_type).into())
      },
    }
  }
}
