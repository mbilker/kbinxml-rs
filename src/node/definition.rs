use std::fmt;

use bytes::Bytes;

use byte_buffer::strip_trailing_null_bytes;
use encoding_type::EncodingType;
use error::{KbinError, KbinErrorKind};
use node::Node;
use node_types::StandardType;
use sixbit::{Sixbit, SixbitSize};
use value::Value;

#[derive(Clone, Eq)]
pub enum Key {
  Compressed {
    size: SixbitSize,
    data: Bytes,
  },
  Uncompressed {
    encoding: EncodingType,
    data: Bytes,
  },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NodeData {
  Some {
    key: Key,
    value_data: Bytes,
  },
  None,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeDefinition {
  encoding: EncodingType,
  pub node_type: StandardType,
  pub is_array: bool,

  data: NodeData,
}

impl Key {
  fn to_string(&self) -> Result<String, KbinError> {
    match self {
      Key::Compressed { ref size, ref data } => {
        Ok(Sixbit::unpack(data, *size)?)
      },
      Key::Uncompressed { encoding, ref data } => {
        Ok(encoding.decode_bytes(data)?)
      },
    }
  }
}

impl NodeDefinition {
  pub fn new(encoding: EncodingType, node_type: (StandardType, bool)) -> Self {
    let (node_type, is_array) = node_type;

    Self {
      encoding,
      node_type,
      is_array,
      data: NodeData::None,
    }
  }

  pub fn with_data(encoding: EncodingType, node_type: (StandardType, bool), data: NodeData) -> Self {
    let (node_type, is_array) = node_type;

    Self {
      encoding,
      node_type,
      is_array,
      data,
    }
  }

  #[inline]
  pub fn encoding(&self) -> EncodingType {
    self.encoding
  }

  #[inline]
  pub fn node_type_tuple(&self) -> (StandardType, bool) {
    (self.node_type, self.is_array)
  }

  pub fn key(&self) -> Result<Option<String>, KbinError> {
    match self.data {
      NodeData::Some { ref key, .. } => key.to_string().map(Some),
      NodeData::None => Ok(None),
    }
  }

  pub fn value(&self) -> Result<Value, KbinError> {
    match (self.node_type, &self.data) {
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

  pub fn value_bytes<'a>(&'a self) -> Option<&'a [u8]> {
    match self.data {
      NodeData::Some { ref value_data, .. } => Some(value_data),
      NodeData::None => None,
    }
  }

  pub fn as_node(&self) -> Result<Node, KbinError> {
    trace!("parsing definition: {:?}", self);
    match (self.node_type, &self.data) {
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
        Ok(Node::with_value(key, value))
      },
      (node_type, NodeData::None) => {
        Err(KbinErrorKind::InvalidNodeType(node_type).into())
      },
    }
  }
}

impl PartialEq for Key {
  fn eq(&self, other: &Key) -> bool {
    match (self.to_string(), other.to_string()) {
      (Ok(key1), Ok(key2)) => {
        key1 == key2
      },
      (_, _) => {
        // If the conversion fails, check if they have the same enum variant
        // to check if the inner data is equal.
        match (self, other) {
          (
            Key::Compressed { data: data1, .. },
            Key::Compressed { data: data2, .. },
          ) => {
            data1 == data2
          },
          (
            Key::Uncompressed { data: data1, .. },
            Key::Uncompressed { data: data2, .. },
          ) => {
            data1 == data2
          },
          (_, _) => false,
        }
      },
    }
  }
}

impl fmt::Debug for Key {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    if let Ok(key) = self.to_string() {
      write!(f, "\"{}\"", key)
    } else {
      match self {
        Key::Compressed { ref size, ref data } => {
          f.debug_struct("Compressed")
            .field("size", &size)
            .field("data", &data)
            .finish()
        },
        Key::Uncompressed { encoding, ref data } => {
          f.debug_struct("Uncompressed")
            .field("encoding", &encoding)
            .field("data", &data)
            .finish()
        },
      }
    }
  }
}

impl fmt::Display for NodeDefinition {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let mut d = f.debug_struct("NodeDefinition");
    d.field("node_type", &self.node_type);

    match self.node_type {
      StandardType::Attribute |
      StandardType::String => {
        d.field("encoding", &self.encoding);
      },
      _ => {},
    };

    match self.data {
      NodeData::Some { ref key, ref value_data } => {
        match key.to_string() {
          Ok(key) => d.field("key", &key),
          Err(e) => d.field("key", &e),
        };
        d.field("value_data", &value_data);
      },
      NodeData::None => {},
    };

    d.finish()
  }
}
