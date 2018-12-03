#![cfg_attr(test, feature(test))]

extern crate byteorder;
extern crate bytes;
extern crate encoding_rs;
extern crate indexmap;
extern crate minidom;
extern crate quick_xml;
extern crate rustc_hex;

#[macro_use] extern crate cfg_if;
#[macro_use] extern crate failure;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate log;

use std::fmt::Write as FmtWrite;

use bytes::Bytes;
use minidom::Element;

mod byte_buffer;
mod compression;
mod encoding_type;
mod error;
mod node;
mod node_types;
mod options;
mod printer;
mod reader;
mod sixbit;
mod text_reader;
mod to_element;
mod to_text_xml;
mod value;
mod writer;

use node::NodeDefinition;
use text_reader::TextXmlReader;
use to_text_xml::TextXmlWriter;

// Public exports
pub use compression::Compression;
pub use encoding_type::EncodingType;
pub use printer::Printer;
pub use reader::Reader;
pub use error::{KbinError, KbinErrorKind, Result};
pub use node::{Node, NodeCollection};
pub use node_types::StandardType;
pub use options::Options;
pub use to_element::ToElement;
pub use to_text_xml::ToTextXml;
pub use value::Value;
pub use writer::{Writer, Writeable};

cfg_if! {
  if #[cfg(feature = "serde")] {
    extern crate serde_bytes;

    #[macro_use] extern crate serde;

    mod de;
    mod ser;

    pub use de::from_bytes;
    pub use node::ExtraNodes;
    pub use ser::to_bytes;
  }
}

const SIGNATURE: u8 = 0xA0;

const SIG_COMPRESSED: u8 = 0x42;
const SIG_UNCOMPRESSED: u8 = 0x45;

const ARRAY_MASK: u8 = 1 << 6; // 1 << 6 = 64

pub fn is_binary_xml(input: &[u8]) -> bool {
  input.len() > 2 && input[0] == SIGNATURE && (input[1] == SIG_COMPRESSED || input[1] == SIG_UNCOMPRESSED)
}

fn read_node(reader: &mut Reader, def: NodeDefinition) -> Result<Element> {
  let key = def.key()?.ok_or(KbinErrorKind::InvalidNodeType(def.node_type))?;
  let mut elem = Element::bare(key);

  // Don't make the assumption that there cannot be a sub-node when a node has a value.
  // Example: `netlog` module
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
        let child = read_node(reader, def)?;
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
        let child = read_node(reader, def)?;
        elem.append_child(child);
      },
    };
  }

  Ok(elem)
}

pub fn element_from_binary(input: &[u8]) -> Result<(Element, EncodingType)> {
  let mut reader = Reader::new(Bytes::from(input))?;
  let base = reader.read_node_definition()?;

  let elem = read_node(&mut reader, base)?;
  let encoding = reader.encoding();

  Ok((elem, encoding))
}

pub fn from_binary(input: Bytes) -> Result<(NodeCollection, EncodingType)> {
  let mut reader = Reader::new(input)?;
  let collection = NodeCollection::from_iter(&mut reader).ok_or(KbinErrorKind::NoNodeCollection)?;
  let encoding = reader.encoding();

  Ok((collection, encoding))
}

pub fn from_text_xml(input: &[u8]) -> Result<(NodeCollection, EncodingType)> {
  let mut reader = TextXmlReader::new(input);
  let collection = reader.as_node_collection()?.ok_or(KbinErrorKind::NoNodeCollection)?;
  let encoding = reader.encoding();

  Ok((collection, encoding))
}

pub fn from_bytes(input: Bytes) -> Result<(NodeCollection, EncodingType)> {
  if is_binary_xml(&input) {
    from_binary(input)
  } else {
    from_text_xml(&input)
  }
}

#[inline]
pub fn from_slice(input: &[u8]) -> Result<(NodeCollection, EncodingType)> {
  from_binary(Bytes::from(input))
}

pub fn to_binary<T>(input: &T) -> Result<Vec<u8>>
  where T: Writeable<T>
{
  let mut writer = Writer::new();
  writer.to_binary(input)
}

pub fn to_binary_with_options<T>(options: Options, input: &T) -> Result<Vec<u8>>
  where T: Writeable<T>
{
  let mut writer = Writer::with_options(options);
  writer.to_binary(input)
}

pub fn to_text_xml<T>(input: &T) -> Result<Vec<u8>>
  where T: ToTextXml
{
  let writer = TextXmlWriter::new();
  writer.to_text_xml(input)
}
