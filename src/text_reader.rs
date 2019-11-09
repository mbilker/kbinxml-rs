use std::num::ParseIntError;
use std::str::{self, Utf8Error};

use bytes::{BufMut, Bytes, BytesMut};
use quick_xml::events::attributes::Attributes;
use quick_xml::events::{BytesStart, BytesText, Event};
use quick_xml::Error as QuickXmlError;
use quick_xml::Reader;
use snafu::{ResultExt, Snafu};

use crate::encoding_type::{EncodingError, EncodingType};
use crate::node::{Key, NodeCollection, NodeData, NodeDefinition};
use crate::node_types::{StandardType, UnknownKbinType};
use crate::value::Value;

const EMPTY_STRING_DATA: &[u8] = &[0];

#[derive(Debug, Snafu)]
pub enum TextReaderError {
    #[snafu(display("Invalid kbin type found"))]
    InvalidKbinType { source: UnknownKbinType },

    #[snafu(display("Invalid encoding type read from declaration"))]
    InvalidEncoding { source: EncodingError },

    #[snafu(display("Failed to parse array count from attribute"))]
    ParseArrayCount { source: ParseIntError },

    #[snafu(display("Failed to parse binary node size from attribute"))]
    ParseBinarySize { source: ParseIntError },

    #[snafu(display(
        "Mismatched binary node length and size attribute value (value length: {}, size attribute: {})",
        len,
        size
    ))]
    MismatchedBinaryNodeLength { len: usize, size: usize },

    #[snafu(display("No node data found"))]
    NoNodeData,

    #[snafu(display("Failed to interpret byte slice as UTF-8"))]
    Utf8 { source: Utf8Error },

    #[snafu(display("Failed to decode value from string for node type {}", node_type))]
    ValueDecode {
        node_type: StandardType,
        #[snafu(source(from(crate::KbinError, Box::new)))]
        source: Box<crate::KbinError>,
    },

    #[snafu(display("Failed to encode value for node type {}", node_type))]
    ValueEncode {
        node_type: StandardType,
        #[snafu(source(from(crate::KbinError, Box::new)))]
        source: Box<crate::KbinError>,
    },

    #[snafu(display("Failed to handle XML operation"))]
    Xml { source: QuickXmlError },
}

impl From<Utf8Error> for TextReaderError {
    #[inline]
    fn from(source: Utf8Error) -> Self {
        Self::Utf8 { source }
    }
}

impl From<QuickXmlError> for TextReaderError {
    #[inline]
    fn from(source: QuickXmlError) -> Self {
        Self::Xml { source }
    }
}

pub struct TextXmlReader<'a> {
    xml_reader: Reader<&'a [u8]>,
    encoding: EncodingType,

    stack: Vec<(NodeCollection, usize, Option<usize>)>,
}

impl<'a> TextXmlReader<'a> {
    pub fn new(input: &'a [u8]) -> Self {
        let mut xml_reader = Reader::from_reader(input);
        xml_reader.trim_text(true);

        Self {
            xml_reader,
            encoding: EncodingType::UTF_8,

            // Most kbinxml files that I have come across do not have too
            // many inner layers.
            stack: Vec::with_capacity(6),
        }
    }

    #[inline]
    pub fn encoding(&self) -> EncodingType {
        self.encoding
    }

    fn parse_attribute(&self, key: &[u8], value: &[u8]) -> Result<NodeDefinition, TextReaderError> {
        let mut value = BytesMut::from(value.to_vec());

        // Add the trailing null byte that kbin has at the end of strings
        value.reserve(1);
        value.put_u8(0);

        let data = NodeData::Some {
            key: Key::Uncompressed {
                encoding: self.encoding,
                data: Bytes::from(key),
            },
            value_data: value.freeze(),
        };

        // `Attribute` nodes do not have the `is_array` flag set
        Ok(NodeDefinition::with_data(
            self.encoding,
            StandardType::Attribute,
            false,
            data,
        ))
    }

    fn parse_attributes(
        &self,
        attrs: Attributes<'a>,
    ) -> Result<(StandardType, usize, Option<usize>, Vec<NodeDefinition>), TextReaderError> {
        let mut node_type = None;
        let mut count = 0;
        let mut size = None;
        let mut attributes = Vec::new();

        for attr in attrs {
            match attr {
                Ok(attr) => {
                    let value = match attr.unescaped_value() {
                        Ok(v) => v,
                        Err(e) => {
                            error!("Error decoding attribute value: {:?}", e);
                            attr.value.clone()
                        },
                    };

                    if attr.key == b"__type" {
                        let value = str::from_utf8(&*value)?;

                        node_type = Some(StandardType::from_name(value).context(InvalidKbinType)?);
                    } else if attr.key == b"__count" {
                        let value = str::from_utf8(&*value)?;
                        let num_count = value.parse::<u32>().context(ParseArrayCount)?;

                        count = num_count as usize;
                    } else if attr.key == b"__size" {
                        let value =
                            str::from_utf8(&*value)?
                                .parse::<usize>()
                                .context(ParseBinarySize)?;

                        size = Some(value);
                    } else {
                        let definition = self.parse_attribute(attr.key, &value)?;
                        attributes.push(definition);
                    }
                },
                Err(e) => {
                    error!("Error reading attribute: {:?}", e);
                },
            };
        }

        let node_type = match node_type {
            Some(node_type) => node_type,
            None => {
                // Default to `NodeStart`, set to `String` if there is a `Event::Text` event before
                // the `Event::End` event.
                StandardType::NodeStart
            },
        };

        Ok((node_type, count, size, attributes))
    }

    fn handle_start(
        &self,
        e: BytesStart,
    ) -> Result<(NodeCollection, usize, Option<usize>), TextReaderError> {
        let (node_type, count, size, attributes) = self.parse_attributes(e.attributes())?;
        let is_array = count > 0;

        // Stub the value for now, handle with `Event::Text`.
        let value_data = match node_type {
            StandardType::String => Bytes::from(EMPTY_STRING_DATA),
            _ => Bytes::new(),
        };
        let data = NodeData::Some {
            key: Key::Uncompressed {
                encoding: self.encoding,
                data: Bytes::from(e.name()),
            },
            value_data,
        };

        let base = NodeDefinition::with_data(self.encoding, node_type, is_array, data);
        let collection = NodeCollection::with_attributes(base, attributes.into());

        Ok((collection, count, size))
    }

    fn handle_text(
        event: BytesText,
        definition: &mut NodeDefinition,
        count: usize,
        size: Option<usize>,
    ) -> Result<(), TextReaderError> {
        let data = event.unescaped()?;
        let data = match definition.node_type {
            StandardType::String | StandardType::NodeStart => {
                let mut data = BytesMut::from(data.into_owned());

                // Add the trailing null byte that kbin has at the end of strings
                data.reserve(1);
                data.put_u8(0);

                data.freeze()
            },
            node_type => {
                let text = str::from_utf8(&*data)?;
                let value = Value::from_string(node_type, text, definition.is_array, count)
                    .context(ValueDecode { node_type })?;

                // The read number of bytes must match the size attribute, if set
                if let Value::Binary(data) = &value {
                    if let Some(size) = size {
                        if data.len() != size {
                            return Err(TextReaderError::MismatchedBinaryNodeLength {
                                len: data.len(),
                                size,
                            });
                        }
                    }
                }

                Bytes::from(value.to_bytes().context(ValueEncode { node_type })?)
            },
        };

        if definition.node_type == StandardType::NodeStart {
            definition.node_type = StandardType::String;
        }

        match definition.data_mut() {
            NodeData::Some {
                ref mut value_data, ..
            } => {
                *value_data = data;
            },
            NodeData::None => {
                // There should be a valid `NodeData` structure from the `Event::Start` handler
                return Err(TextReaderError::NoNodeData);
            },
        };

        Ok(())
    }

    pub fn as_node_collection(&mut self) -> Result<Option<NodeCollection>, TextReaderError> {
        // A buffer size for reading a `quick_xml::events::Event` that I pulled
        // out of my head.
        let mut buf = Vec::with_capacity(1024);

        loop {
            match self.xml_reader.read_event(&mut buf)? {
                Event::Start(e) => {
                    let start = self.handle_start(e)?;
                    self.stack.push(start);
                },
                Event::Text(e) => {
                    if let Some((ref mut collection, ref count, ref size)) = self.stack.last_mut() {
                        let base = collection.base_mut();
                        Self::handle_text(e, base, *count, *size)?;
                    }
                },
                Event::End(_) => {
                    if let Some((collection, _count, _size)) = self.stack.pop() {
                        if let Some((parent_collection, _count, _size)) = self.stack.last_mut() {
                            parent_collection.children_mut().push_back(collection);
                        } else {
                            // The end of the structure has been reached.
                            return Ok(Some(collection));
                        }
                    }
                },
                Event::Empty(e) => {
                    let (collection, count, size) = self.handle_start(e)?;
                    assert!(count == 0, "empty node should not signal an array");
                    assert!(
                        size.is_none() || size == Some(0),
                        "empty node should not signal binary data"
                    );

                    if let Some((ref mut parent_collection, _count, _size)) = self.stack.last_mut()
                    {
                        parent_collection.children_mut().push_back(collection);
                    }
                },
                Event::Decl(e) => {
                    if let Some(encoding) = e.encoding() {
                        self.encoding =
                            EncodingType::from_label(&encoding?).context(InvalidEncoding)?;
                    }
                },
                Event::Eof => break,
                _ => {},
            };

            buf.clear();
        }

        Ok(None)
    }
}
