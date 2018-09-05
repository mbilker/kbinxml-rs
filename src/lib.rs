#![feature(int_to_from_bytes)]

#![cfg_attr(test, feature(test))]

extern crate byteorder;
extern crate encoding;
extern crate indexmap;
extern crate minidom;
extern crate rustc_hex;
extern crate serde_bytes;

#[macro_use] extern crate failure;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate log;
#[macro_use] extern crate serde;

use std::fmt::Write as FmtWrite;
use std::io::{Cursor, Write};

use byteorder::{BigEndian, WriteBytesExt};
use failure::ResultExt;
use minidom::Element;
use rustc_hex::FromHex;

mod byte_buffer;
mod compression;
mod encoding_type;
mod error;
mod ip4;
mod kbin_wrapper;
mod node;
mod node_types;
mod options;
mod printer;
mod reader;
mod sixbit;
mod value;

mod de;
mod ser;

use byte_buffer::ByteBufferWrite;
use node::NodeDefinition;
use node_types::StandardType;
use reader::Reader;
use sixbit::Sixbit;

// Public exports
pub use encoding_type::EncodingType;
pub use printer::Printer;
pub use error::{KbinError, KbinErrorKind, Result};
pub use node::{ExtraNodes, Node};
pub use options::Options;
pub use de::from_bytes;
pub use ip4::Ip4Addr;
pub use ser::to_bytes;
pub use value::Value;

const SIGNATURE: u8 = 0xA0;

const SIG_COMPRESSED: u8 = 0x42;
const SIG_UNCOMPRESSED: u8 = 0x45;

const ARRAY_MASK: u8 = 1 << 6; // 1 << 6 = 64

pub struct KbinXml {
  options: Options,
}

impl KbinXml {
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

  pub fn is_binary_xml(input: &[u8]) -> bool {
    input.len() > 2 && input[0] == SIGNATURE && (input[1] == SIG_COMPRESSED || input[1] == SIG_UNCOMPRESSED)
  }

  fn read_node(&mut self, reader: &mut Reader, def: NodeDefinition) -> Result<Element> {
    let key = def.key()?.ok_or(KbinErrorKind::InvalidNodeType(def.node_type))?;
    let mut elem = Element::bare(key);

    if def.node_type != StandardType::NodeStart {
      elem.set_attr("__type", def.node_type.name);

      match def.value()? {
        Value::Binary(data) => {
          elem.set_attr("__size", data.len());

          let len = data.len() * 2;
          let value = data.into_iter().fold(String::with_capacity(len), |mut val, x| {
            write!(val, "{:02x}", x).expect("Failed to append hex char");
            val
          });
          debug!("KbinXml::read_node(name: {}) => binary value: {}", elem.name(), value);
          elem.append_text_node(value);
        },
        Value::String(value) => {
          debug!("KbinXml::read_node(name: {}) => string value: {:?}", elem.name(), value);
          elem.append_text_node(value);
        },
        Value::Array(node_type, values) => {
          elem.set_attr("__count", values.len());

          let value = Value::Array(node_type, values).to_string();
          debug!("KbinXml::read_node(name: {}) => value: {:?}", elem.name(), value);
          elem.append_text_node(value);
        },
        value => {
          let value = value.to_string();
          debug!("KbinXml::read_node(name: {}) => value: {:?}", elem.name(), value);
          elem.append_text_node(value);
        },
      }
    }

    loop {
      let def = reader.read_node_definition()?;

      match def.node_type {
        StandardType::NodeEnd => break,
        StandardType::NodeStart => {
          let child = self.read_node(reader, def)?;
          elem.append_child(child);

          continue;
        },
        StandardType::Attribute => {
          let node = def.as_node()?;
          let (key, value) = node.into_key_and_value();
          if let Some(Value::Attribute(value)) = value {
            elem.set_attr(key, value);
          } else {
            return Err(KbinErrorKind::InvalidState.into());
          }
        },
        _ => {
          let child = self.read_node(reader, def)?;
          elem.append_child(child);
        },
      };
    }

    Ok(elem)
  }

  fn from_binary_internal(&mut self, input: &[u8]) -> Result<(Element, EncodingType)> {
    let mut reader = Reader::new(input)?;
    let base = reader.read_node_definition()?;

    let elem = self.read_node(&mut reader, base)?;
    let encoding = reader.encoding();

    Ok((elem, encoding))
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

    // TODO: support uncompressed
    node_buf.write_u8(node_type.id | array_mask).context(KbinErrorKind::DataWrite(node_type.name))?;
    Sixbit::pack(&mut **node_buf, input.name())?;

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
        let data = node_type.to_bytes(&text, count as usize)?;
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
      Sixbit::pack(&mut **node_buf, key)?;
    }

    for child in input.children() {
      self.write_node(node_buf, data_buf, child)?;
    }

    // Always has the array bit set
    node_buf.write_u8(StandardType::NodeEnd.id | ARRAY_MASK).context(KbinErrorKind::DataWrite("node end"))?;

    Ok(())
  }

  fn to_binary_internal(&mut self, input: &Element) -> Result<Vec<u8>> {
    let mut header = Cursor::new(Vec::with_capacity(8));
    header.write_u8(SIGNATURE).context(KbinErrorKind::HeaderWrite("signature"))?;
    header.write_u8(SIG_COMPRESSED).context(KbinErrorKind::HeaderWrite("compression"))?;

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

  pub fn from_binary(input: &[u8]) -> Result<(Element, EncodingType)> {
    let mut kbinxml = KbinXml::new();
    kbinxml.from_binary_internal(input)
  }

  pub fn to_binary(input: &Element) -> Result<Vec<u8>> {
    let mut kbinxml = KbinXml::new();
    kbinxml.to_binary_internal(input)
  }

  pub fn to_binary_with_options(options: Options, input: &Element) -> Result<Vec<u8>> {
    let mut kbinxml = KbinXml::with_options(options);
    kbinxml.to_binary_internal(input)
  }
}
