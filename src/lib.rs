#![cfg_attr(test, feature(test))]

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

use bytes::Bytes;

mod byte_buffer;
mod compression_type;
mod encoding_type;
mod error;
mod node;
mod node_types;
mod options;
mod printer;
mod reader;
mod sixbit;
mod text_reader;
mod to_text_xml;
mod types;
mod value;
mod writer;

use crate::text_reader::TextXmlReader;
use crate::to_text_xml::TextXmlWriter;

// Public exports
pub use crate::compression_type::CompressionType;
pub use crate::encoding_type::EncodingType;
pub use crate::error::{KbinError, Result};
pub use crate::node::{Node, NodeCollection};
pub use crate::node_types::StandardType;
pub use crate::options::{Options, OptionsBuilder};
pub use crate::printer::Printer;
pub use crate::reader::Reader;
pub use crate::to_text_xml::ToTextXml;
pub use crate::value::{Value, ValueArray};
pub use crate::writer::{Writeable, Writer};

const SIGNATURE: u8 = 0xA0;

const SIG_COMPRESSED: u8 = 0x42;
const SIG_UNCOMPRESSED: u8 = 0x45;

const ARRAY_MASK: u8 = 1 << 6; // 1 << 6 = 64

pub fn is_binary_xml(input: &[u8]) -> bool {
    input.len() > 2 &&
        input[0] == SIGNATURE &&
        (input[1] == SIG_COMPRESSED || input[1] == SIG_UNCOMPRESSED)
}

pub fn from_binary(input: Bytes) -> Result<(NodeCollection, EncodingType)> {
    let mut reader = Reader::new(input)?;
    let collection = NodeCollection::from_iter(&mut reader).ok_or(KbinError::NoNodeCollection)?;
    let encoding = reader.encoding();

    Ok((collection, encoding))
}

pub fn from_text_xml(input: &[u8]) -> Result<(NodeCollection, EncodingType)> {
    let mut reader = TextXmlReader::new(input);
    let collection = reader
        .as_node_collection()?
        .ok_or(KbinError::NoNodeCollection)?;
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
where
    T: Writeable,
{
    let mut writer = Writer::new();
    writer.to_binary(input)
}

pub fn to_binary_with_options<T>(options: Options, input: &T) -> Result<Vec<u8>>
where
    T: Writeable,
{
    let mut writer = Writer::with_options(options);
    writer.to_binary(input)
}

pub fn to_text_xml<T>(input: &T) -> Result<Vec<u8>>
where
    T: ToTextXml,
{
    let writer = TextXmlWriter::new();
    writer.to_text_xml(input)
}
