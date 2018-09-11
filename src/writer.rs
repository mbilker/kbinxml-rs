use std::io::{Cursor, Write};

use byteorder::{BigEndian, WriteBytesExt};
use failure::ResultExt;
use minidom::Element;
use rustc_hex::FromHex;
use byte_buffer::ByteBufferWrite;
use node_types::StandardType;
use sixbit::Sixbit;

use compression::Compression;
use error::{KbinErrorKind, Result};
use options::Options;
use value::Value;

use super::{ARRAY_MASK, SIGNATURE};

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

  fn write_node(&mut self, node_buf: &mut ByteBufferWrite, data_buf: &mut ByteBufferWrite, input: &Element) -> Result<()> {
    let text = input.text();
    let node_type = match input.attr("__type") {
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

    let (array_mask, count) = match input.attr("__count") {
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
      input.name(),
      node_type,
      node_type.size,
      node_type.count,
      array_mask,
      count);

    node_buf.write_u8(node_type.id | array_mask).context(KbinErrorKind::DataWrite(node_type.name))?;
    match self.options.compression {
      Compression::Compressed => Sixbit::pack(&mut **node_buf, input.name())?,
      Compression::Uncompressed => {
        let data = self.options.encoding.encode_bytes(input.name())?;
        let len = (data.len() - 1) as u8;
        node_buf.write_u8(len | ARRAY_MASK).context(KbinErrorKind::DataWrite("node name length"))?;
        node_buf.write_all(&data).context(KbinErrorKind::DataWrite("node name bytes"))?;
      },
    };

    match node_type {
      StandardType::NodeStart => {},

      StandardType::Binary => {
        let data: Vec<u8> = text.from_hex().context(KbinErrorKind::HexError)?;
        trace!("data: 0x{:02x?}", data);

        let size = (data.len() as u32) * (node_type.size as u32);
        data_buf.write_u32::<BigEndian>(size).context(KbinErrorKind::DataWrite("binary node size"))?;
        data_buf.write_all(&data).context(KbinErrorKind::DataWrite("binary"))?;
        data_buf.realign_writes(None)?;
      },
      StandardType::String => {
        data_buf.write_str(self.options.encoding, &text)?;
      },

      _ => {
        let value = Value::from_string(node_type, &text, array_mask > 0, count as usize)?;
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

    for (key, value) in input.attrs() {
      match key {
        "__count" | "__size" | "__type" => continue,
        _ => {},
      };

      trace!("write_node => attr: {}, value: {}", key, value);

      data_buf.write_str(self.options.encoding, value)?;

      let node_type = StandardType::Attribute;
      node_buf.write_u8(node_type.id).context(KbinErrorKind::DataWrite(node_type.name))?;
      match self.options.compression {
        Compression::Compressed => Sixbit::pack(&mut **node_buf, key)?,
        Compression::Uncompressed => {
          let data = self.options.encoding.encode_bytes(key)?;
          let len = (data.len() - 1) as u8;
          node_buf.write_u8(len | ARRAY_MASK).context(KbinErrorKind::DataWrite("attribute name length"))?;
          node_buf.write_all(&data).context(KbinErrorKind::DataWrite("node name bytes"))?;
        },
      };
    }

    for child in input.children() {
      self.write_node(node_buf, data_buf, child)?;
    }

    // Always has the array bit set
    node_buf.write_u8(StandardType::NodeEnd.id | ARRAY_MASK).context(KbinErrorKind::DataWrite("node end"))?;

    Ok(())
  }

  pub fn to_binary(&mut self, input: &Element) -> Result<Vec<u8>> {
    let mut header = Cursor::new(Vec::with_capacity(8));
    header.write_u8(SIGNATURE).context(KbinErrorKind::HeaderWrite("signature"))?;

    let compression = self.options.compression.to_byte();
    header.write_u8(compression).context(KbinErrorKind::HeaderWrite("compression"))?;

    let encoding = self.options.encoding.to_byte();
    header.write_u8(encoding).context(KbinErrorKind::HeaderWrite("encoding"))?;
    header.write_u8(0xFF ^ encoding).context(KbinErrorKind::HeaderWrite("encoding negation"))?;

    let mut node_buf = ByteBufferWrite::new(Vec::new());
    let mut data_buf = ByteBufferWrite::new(Vec::new());

    self.write_node(&mut node_buf, &mut data_buf, input)?;

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
