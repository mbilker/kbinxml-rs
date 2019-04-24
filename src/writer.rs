use std::io::{Cursor, Write};

use byteorder::{BigEndian, WriteBytesExt};
use failure::ResultExt;
use minidom::Element;
use rustc_hex::FromHex;

use crate::byte_buffer::ByteBufferWrite;
use crate::compression::Compression;
use crate::error::{KbinErrorKind, Result};
use crate::node::{Node, NodeCollection};
use crate::node_types::StandardType;
use crate::options::Options;
use crate::sixbit::Sixbit;
use crate::value::Value;

use super::{ARRAY_MASK, SIGNATURE};

fn write_value(options: &Options, data_buf: &mut ByteBufferWrite, node_type: StandardType, is_array: bool, value: &Value) -> Result<()> {
  match value {
    Value::Binary(data) => {
      trace!("data: 0x{:02x?}", data);

      let size = (data.len() as u32) * (node_type.size as u32);
      data_buf.write_u32::<BigEndian>(size).context(KbinErrorKind::DataWrite("binary node size"))?;
      data_buf.write_all(&data).context(KbinErrorKind::DataWrite("binary"))?;
      data_buf.realign_writes(None)?;
    },
    Value::String(text) => {
      data_buf.write_str(options.encoding, &text)?;
    },
    Value::Array(node_type, values) => {
      if !is_array {
        return Err(KbinErrorKind::InvalidState.into());
      }

      let count = values.len();
      let total_size = count * node_type.count * node_type.size;

      let mut data = Vec::with_capacity(total_size);
      for value in values {
        value.to_bytes_into(&mut data)?;
      }

      data_buf.write_u32::<BigEndian>(total_size as u32).context(KbinErrorKind::DataWrite("node size"))?;
      data_buf.write_all(&data).context(KbinErrorKind::DataWrite(node_type.name))?;
      data_buf.realign_writes(None)?;
    },
    value => {
      if is_array {
        return Err(KbinErrorKind::InvalidState.into());
      } else {
        let data = value.to_bytes()?;
        data_buf.write_aligned(*node_type, &data)?;
      }
    },
  };

  Ok(())
}

pub trait Writeable {
  fn write_node(&self, options: &Options, node_buf: &mut ByteBufferWrite, data_buf: &mut ByteBufferWrite) -> Result<()>;
}

impl Writeable for Element {
  fn write_node(&self, options: &Options, node_buf: &mut ByteBufferWrite, data_buf: &mut ByteBufferWrite) -> Result<()> {
    let text = self.text();
    let node_type = match self.attr("__type") {
      Some(name) => StandardType::from_name(name),
      None => {
        // Screw whitespace with pretty printed XML
        if text.trim().len() == 0 {
          StandardType::NodeStart
        } else {
          StandardType::String
        }
      },
    };

    let (array_mask, count) = match self.attr("__count") {
      Some(count) => {
        let count = count.parse::<u32>().context(KbinErrorKind::StringParse("array count"))?;
        debug!("write_node => __count = {}", count);
        (ARRAY_MASK, count)
      },
      None => {
        (0, 1)
      },
    };

    debug!("write_node => name: {}, type: {:?}, type_size: {}, type_count: {}, is_array: {}, size: {}",
      self.name(),
      node_type,
      node_type.size,
      node_type.count,
      array_mask,
      count);

    node_buf.write_u8(node_type.id | array_mask).context(KbinErrorKind::DataWrite(node_type.name))?;
    match options.compression {
      Compression::Compressed => Sixbit::pack(&mut **node_buf, self.name())?,
      Compression::Uncompressed => {
        let data = options.encoding.encode_bytes(self.name())?;
        let len = (data.len() - 1) as u8;
        node_buf.write_u8(len | ARRAY_MASK).context(KbinErrorKind::DataWrite("node name length"))?;
        node_buf.write_all(&data).context(KbinErrorKind::DataWrite("node name bytes"))?;
      },
    };

    match node_type {
      StandardType::NodeStart => {},

      StandardType::Binary => {
        let data: Vec<u8> = text.from_hex().context(KbinErrorKind::HexError)?;
        let size = (data.len() as u32) * (node_type.size as u32);
        data_buf.write_u32::<BigEndian>(size).context(KbinErrorKind::DataWrite("binary node size"))?;
        data_buf.write_all(&data).context(KbinErrorKind::DataWrite("binary"))?;
        data_buf.realign_writes(None)?;
      },
      StandardType::String => {
        data_buf.write_str(options.encoding, &text)?;
      },

      _ => {
        let value = Value::from_string(node_type, text.trim(), array_mask > 0, count as usize)?;
        let data = value.to_bytes()?;

        if array_mask > 0 {
          let total_size = (count as u32) * (node_type.count as u32) * (node_type.size as u32);
          trace!("write_node data_buf array => total_size: {}, data: 0x{:02x?}", total_size, data);

          data_buf.write_u32::<BigEndian>(total_size).context(KbinErrorKind::DataWrite("node size"))?;
          data_buf.write_all(&data).context(KbinErrorKind::DataWrite(node_type.name))?;
          data_buf.realign_writes(None)?;
        } else {
          data_buf.write_aligned(*node_type, &data)?;
        }
      },
    }

    for (key, value) in self.attrs() {
      match key {
        "__count" | "__size" | "__type" => continue,
        _ => {},
      };

      trace!("write_node => attr: {}, value: {}", key, value);

      data_buf.write_str(options.encoding, value)?;

      let node_type = StandardType::Attribute;
      node_buf.write_u8(node_type.id).context(KbinErrorKind::DataWrite(node_type.name))?;
      match options.compression {
        Compression::Compressed => Sixbit::pack(&mut **node_buf, key)?,
        Compression::Uncompressed => {
          let data = options.encoding.encode_bytes(key)?;
          let len = (data.len() - 1) as u8;
          node_buf.write_u8(len | ARRAY_MASK).context(KbinErrorKind::DataWrite("attribute name length"))?;
          node_buf.write_all(&data).context(KbinErrorKind::DataWrite("node name bytes"))?;
        },
      };
    }

    for child in self.children() {
      child.write_node(options, node_buf, data_buf)?;
    }

    // Always has the array bit set
    node_buf.write_u8(StandardType::NodeEnd.id | ARRAY_MASK).context(KbinErrorKind::DataWrite("node end"))?;

    Ok(())
  }
}

impl Writeable for NodeCollection {
  fn write_node(&self, options: &Options, node_buf: &mut ByteBufferWrite, data_buf: &mut ByteBufferWrite) -> Result<()> {
    let (node_type, is_array) = self.base().node_type_tuple();
    let array_mask = if is_array { ARRAY_MASK } else { 0 };
    let name = self.base().key()?.ok_or(KbinErrorKind::InvalidState)?;

    debug!("NodeCollection write_node => name: {}, type: {:?}, type_size: {}, type_count: {}, is_array: {}",
      name,
      node_type,
      node_type.size,
      node_type.count,
      is_array);

    node_buf.write_u8(node_type.id | array_mask).context(KbinErrorKind::DataWrite(node_type.name))?;
    match options.compression {
      Compression::Compressed => Sixbit::pack(&mut **node_buf, &name)?,
      Compression::Uncompressed => {
        let data = options.encoding.encode_bytes(&name)?;
        let len = (data.len() - 1) as u8;
        node_buf.write_u8(len | ARRAY_MASK).context(KbinErrorKind::DataWrite("node name length"))?;
        node_buf.write_all(&data).context(KbinErrorKind::DataWrite("node name bytes"))?;
      },
    };

    if node_type != StandardType::NodeStart {
      let value = self.base().value()?;
      write_value(options, data_buf, node_type, is_array, &value)?;
    }

    for attr in self.attributes() {
      let key = attr.key()?.ok_or(KbinErrorKind::InvalidState)?;
      let value = attr.value_bytes().ok_or(KbinErrorKind::InvalidState)?;

      trace!("NodeCollection write_node => attr: {}, value: 0x{:02x?}", key, value);

      data_buf.buf_write(value)?;

      node_buf.write_u8(StandardType::Attribute.id).context(KbinErrorKind::DataWrite(StandardType::Attribute.name))?;
      match options.compression {
        Compression::Compressed => Sixbit::pack(&mut **node_buf, &key)?,
        Compression::Uncompressed => {
          let data = options.encoding.encode_bytes(&key)?;
          let len = (data.len() - 1) as u8;
          node_buf.write_u8(len | ARRAY_MASK).context(KbinErrorKind::DataWrite("attribute name length"))?;
          node_buf.write_all(&data).context(KbinErrorKind::DataWrite("node name bytes"))?;
        },
      };
    }

    for child in self.children() {
      child.write_node(options, node_buf, data_buf)?;
    }

    // Always has the array bit set
    node_buf.write_u8(StandardType::NodeEnd.id | ARRAY_MASK).context(KbinErrorKind::DataWrite("node end"))?;

    Ok(())
  }
}

impl Writeable for Node {
  fn write_node(&self, options: &Options, node_buf: &mut ByteBufferWrite, data_buf: &mut ByteBufferWrite) -> Result<()> {
    let (node_type, is_array) = match self.value() {
      Some(Value::Array(node_type, _)) => (*node_type, true),
      Some(ref value) => (value.standard_type(), false),
      None => (StandardType::NodeStart, false),
    };
    let array_mask = if is_array { ARRAY_MASK } else { 0 };

    debug!("Node write_node => name: {}, type: {:?}, type_size: {}, type_count: {}, is_array: {}",
      self.key(),
      node_type,
      node_type.size,
      node_type.count,
      is_array);

    node_buf.write_u8(node_type.id | array_mask).context(KbinErrorKind::DataWrite(node_type.name))?;
    match options.compression {
      Compression::Compressed => Sixbit::pack(&mut **node_buf, &self.key())?,
      Compression::Uncompressed => {
        let data = options.encoding.encode_bytes(&self.key())?;
        let len = (data.len() - 1) as u8;
        node_buf.write_u8(len | ARRAY_MASK).context(KbinErrorKind::DataWrite("node name length"))?;
        node_buf.write_all(&data).context(KbinErrorKind::DataWrite("node name bytes"))?;
      },
    };

    if let Some(value) = self.value() {
      write_value(options, data_buf, node_type, is_array, value)?;
    }

    if let Some(attributes) = self.attributes() {
      for (key, value) in attributes {
        trace!("Node write_node => attr: {}, value: {}", key, value);

        data_buf.write_str(options.encoding, value)?;

        node_buf.write_u8(StandardType::Attribute.id).context(KbinErrorKind::DataWrite(StandardType::Attribute.name))?;
        match options.compression {
          Compression::Compressed => Sixbit::pack(&mut **node_buf, &key)?,
          Compression::Uncompressed => {
            let data = options.encoding.encode_bytes(&key)?;
            let len = (data.len() - 1) as u8;
            node_buf.write_u8(len | ARRAY_MASK).context(KbinErrorKind::DataWrite("attribute name length"))?;
            node_buf.write_all(&data).context(KbinErrorKind::DataWrite("node name bytes"))?;
          },
        };
      }
    }

    if let Some(children) = self.children() {
      for child in children {
        child.write_node(options, node_buf, data_buf)?;
      }
    }

    // Always has the array bit set
    node_buf.write_u8(StandardType::NodeEnd.id | ARRAY_MASK).context(KbinErrorKind::DataWrite("node end"))?;

    Ok(())
  }
}

pub struct Writer {
  options: Options,
}

impl Writer {
  pub fn new() -> Self {
    Self {
      options: Options::default(),
    }
  }

  pub fn with_options(options: Options) -> Self {
    Self {
      options,
    }
  }

  pub fn to_binary<T>(&mut self, input: &T) -> Result<Vec<u8>>
    where T: Writeable
  {
    let mut header = Cursor::new(Vec::with_capacity(8));
    header.write_u8(SIGNATURE).context(KbinErrorKind::HeaderWrite("signature"))?;

    let compression = self.options.compression.to_byte();
    header.write_u8(compression).context(KbinErrorKind::HeaderWrite("compression"))?;

    let encoding = self.options.encoding.to_byte();
    header.write_u8(encoding).context(KbinErrorKind::HeaderWrite("encoding"))?;
    header.write_u8(0xFF ^ encoding).context(KbinErrorKind::HeaderWrite("encoding negation"))?;

    let mut node_buf = ByteBufferWrite::new(Vec::new());
    let mut data_buf = ByteBufferWrite::new(Vec::new());

    input.write_node(&self.options, &mut node_buf, &mut data_buf)?;

    node_buf.write_u8(StandardType::FileEnd.id | ARRAY_MASK).context(KbinErrorKind::DataWrite("file end"))?;
    node_buf.realign_writes(None)?;

    let mut output = header.into_inner();

    let node_buf = node_buf.into_inner();
    debug!("to_binary_internal => node_buf len: {0} (0x{0:x})", node_buf.len());
    output.write_u32::<BigEndian>(node_buf.len() as u32).context(KbinErrorKind::HeaderWrite("node buffer length"))?;
    output.extend_from_slice(&node_buf);

    let data_buf = data_buf.into_inner();
    debug!("to_binary_internal => data_buf len: {0} (0x{0:x})", data_buf.len());
    output.write_u32::<BigEndian>(data_buf.len() as u32).context(KbinErrorKind::HeaderWrite("data buffer length"))?;
    output.extend_from_slice(&data_buf);

    Ok(output)
  }
}
