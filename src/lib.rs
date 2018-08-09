#![feature(int_to_from_bytes)]

extern crate byteorder;
extern crate encoding;
extern crate indexmap;
extern crate minidom;
extern crate num;
extern crate rustc_hex;
extern crate serde;

#[macro_use] extern crate failure;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate log;

#[cfg(test)]
#[macro_use] extern crate serde_derive;

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
mod node_types;
mod options;
mod printer;
mod reader;
mod sixbit;
mod value;

mod de;
mod ser;

use byte_buffer::ByteBufferWrite;
use node_types::StandardType;
use reader::Reader;
use sixbit::pack_sixbit;

// Public exports
pub use encoding_type::EncodingType;
pub use printer::Printer;
pub use error::{KbinError, KbinErrorKind, Result};
pub use options::Options;
pub use de::from_bytes;
pub use ip4::Ip4Addr;
pub use ser::to_bytes;
pub use value::Value;

const SIGNATURE: u8 = 0xA0;

const SIG_COMPRESSED: u8 = 0x42;

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
    input.len() > 2 && input[0] == SIGNATURE && input[1] == SIG_COMPRESSED
  }

  fn from_binary_internal(&mut self, stack: &mut Vec<Element>, input: &[u8]) -> Result<(Element, EncodingType)> {
    let mut reader = Reader::new(input)?;

    while reader.node_buf.position() < reader.data_buf_start() {
      let (xml_type, is_array) = reader.read_node_type()?;

      match xml_type {
        StandardType::NodeEnd |
        StandardType::FileEnd => {
          if stack.len() > 1 {
            let node = stack.pop().expect("Stack must have last node");
            if let Some(to) = stack.last_mut() {
              to.append_child(node);
            }
          }

          if xml_type == StandardType::NodeEnd {
            continue;
          } else if xml_type == StandardType::FileEnd {
            break;
          }
        },
        _ => {},
      };

      let name = reader.read_node_identifier()?;

      if xml_type == StandardType::NodeStart {
        stack.push(Element::bare(name));
        continue;
      }

      if xml_type != StandardType::Attribute {
        stack.push(Element::bare(name.clone()));
      }
      if let Some(to) = stack.last_mut() {
        match xml_type {
          StandardType::Attribute => {
            let val = reader.read_string()?;
            debug!("attr name: {}, val: {}", name, val);
            to.set_attr(name, val);
          },
          StandardType::Binary => {
            to.set_attr("__type", xml_type.name);

            let data = reader.read_bytes().context(KbinErrorKind::BinaryLengthRead)?;
            to.set_attr("__size", data.len());

            let len = data.len() * 2;
            let val = data.into_iter().fold(String::with_capacity(len), |mut val, x| {
              write!(val, "{:02x}", x).expect("Failed to append hex char");
              val
            });
            debug!("name: {}, string: {}", name, val);
            to.append_text_node(val);
          },
          // Removing null bytes is *so much* fun.
          //
          // Handle String nodes separately to use the string reading logic
          // which automatically removes trailing null bytes.
          StandardType::String => {
            to.set_attr("__type", xml_type.name);

            let val = reader.read_string()?;
            debug!("type: {:?}, is_array: {}, name: {}, val: {}", xml_type, is_array, name, val);
            to.append_text_node(val);
          },
          _ => {
            to.set_attr("__type", xml_type.name);

            let type_size = xml_type.size;
            let type_count = xml_type.count;
            let (is_array, size) = if is_array {
              let node_size = type_size * type_count;
              let arr_count = reader.read_u32().context(KbinErrorKind::ArrayLengthRead)? / node_size as u32;
              to.set_attr("__count", arr_count);

              let size = (node_size as u32) * arr_count;
              (true, size)
            } else {
              (false, 1)
            };

            debug!("type: {:?}, type_size: {}, type_count: {}, is_array: {}, size: {}",
              xml_type,
              type_size,
              type_count,
              is_array,
              size);

            let data = if is_array {
              let data = reader.data_buf.get(size)?;
              reader.data_buf.realign_reads(None)?;

              data
            } else {
              reader.data_buf.get_aligned(*xml_type)?
            };
            debug!("data: 0x{:02x?}", data);

            let inner_value = xml_type.parse_bytes(&data)?;
            debug!("name: {}, string: {}", name, inner_value);
            to.append_text_node(inner_value);
          },
        };
      }
    }

    if stack.len() > 1 {
      warn!("stack: {:#?}", stack);
    }
    stack.truncate(1);

    let element = stack.pop().expect("Stack must have root node");
    let encoding = reader.encoding();
    Ok((element, encoding))
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
    pack_sixbit(&mut **node_buf, input.name())?;

    match node_type {
      StandardType::NodeStart => {},

      StandardType::Binary => {
        let data = text.from_hex().context(KbinErrorKind::HexError)?;
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
      pack_sixbit(&mut **node_buf, key)?;
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
    let mut stack: Vec<Element> = Vec::new();

    kbinxml.from_binary_internal(&mut stack, input).map_err(|e| {
      if let Some(first) = stack.first() {
        println!("{:?}", first);
      }
      e
    })
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
