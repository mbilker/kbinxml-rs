use std::io::{self, Cursor, Write};

use byteorder::{BigEndian, WriteBytesExt};
use snafu::{ResultExt, Snafu};

use crate::byte_buffer::{ByteBufferError, ByteBufferWrite};
use crate::compression_type::CompressionType;
use crate::encoding_type::{EncodingError, EncodingType};
use crate::node::{Node, NodeCollection};
use crate::node_types::StandardType;
use crate::options::Options;
use crate::sixbit::{Sixbit, SixbitError};
use crate::value::Value;

use super::{ARRAY_MASK, SIGNATURE};

#[derive(Debug, Snafu)]
pub enum WriterError {
    #[snafu(display("Failed to write signature to header"))]
    Signature { source: io::Error },

    #[snafu(display("Failed to write compression type to header"))]
    Compression { source: io::Error },

    #[snafu(display("Failed to write encoding type to header"))]
    Encoding { source: io::Error },

    #[snafu(display("Failed to write encoding type inverted value to header"))]
    EncodingNegate { source: io::Error },

    #[snafu(display("Failed to write node buffer length"))]
    NodeBufferLength { source: io::Error },

    #[snafu(display("Failed to write data buffer length"))]
    DataBufferLength { source: io::Error },

    #[snafu(display(
        "Failed to write node size ({} byte(s)) for node type {}",
        size,
        node_type
    ))]
    NodeSize {
        node_type: StandardType,
        size: u32,
        source: io::Error,
    },

    #[snafu(display("Failed to write node data for node type {}", node_type))]
    DataWrite {
        node_type: StandardType,
        source: io::Error,
    },

    #[snafu(display("Failed to write sixbit node name"))]
    NodeSixbitName { source: SixbitError },

    #[snafu(display("Failed to encode uncompressed node name to {:?}", encoding))]
    NodeUncompressedNameEncode {
        encoding: EncodingType,
        source: EncodingError,
    },

    #[snafu(display("Failed to write uncompressed node name length"))]
    NodeUncompressedNameLength { source: io::Error },

    #[snafu(display("Failed to write uncompressed node name data"))]
    NodeUncompressedNameData { source: io::Error },

    #[snafu(display("Failed to write node type {} to node buffer", node_type))]
    NodeType {
        node_type: StandardType,
        source: io::Error,
    },

    #[snafu(display("Failed to handle data buffer operation for node type {}", node_type))]
    DataBuffer {
        node_type: StandardType,
        source: ByteBufferError,
    },

    #[snafu(display("Failed to handle node buffer operation for node type {}", node_type))]
    NodeBuffer {
        node_type: StandardType,
        source: ByteBufferError,
    },

    // TODO: remove when better error type is made
    #[snafu(display("Failed to encode value to bytes for node type {}", node_type))]
    ValueEncode {
        node_type: StandardType,
        #[snafu(source(from(crate::KbinError, Box::new)))]
        source: Box<crate::KbinError>,
    },

    // TODO: remove when better error type is made
    #[snafu(display("Failed to get key from definition for node type {}", node_type))]
    DefinitionKey {
        node_type: StandardType,
        #[snafu(source(from(crate::KbinError, Box::new)))]
        source: Box<crate::KbinError>,
    },

    // TODO: remove when better error type is made
    #[snafu(display("Failed to get value from definition for node type {}", node_type))]
    DefinitionValue {
        node_type: StandardType,
        #[snafu(source(from(crate::KbinError, Box::new)))]
        source: Box<crate::KbinError>,
    },

    #[snafu(display("Attempted to write node definition without key data"))]
    NoNodeKey,

    #[snafu(display("Attempted to write node definition without value data"))]
    NoNodeValue,
}

fn write_value(
    options: &Options,
    data_buf: &mut ByteBufferWrite,
    node_type: StandardType,
    is_array: bool,
    value: &Value,
) -> Result<(), WriterError> {
    match value {
        Value::Binary(data) => {
            trace!("data: 0x{:02x?}", data);

            // TODO: add overflow check
            let size = (data.len() * node_type.size) as u32;
            data_buf
                .write_u32::<BigEndian>(size)
                .context(NodeSize { node_type, size })?;
            data_buf.write_all(&data).context(DataWrite { node_type })?;
            data_buf
                .realign_writes(None)
                .context(DataBuffer { node_type })?;
        },
        Value::String(text) => {
            data_buf
                .write_str(options.encoding, &text)
                .context(DataBuffer { node_type })?;
        },
        Value::Array(values) => {
            if !is_array {
                panic!("Attempted to write value array but was not marked as array");
            }

            let total_size = values.len() * node_type.count * node_type.size;

            let mut data = Vec::with_capacity(total_size);
            values
                .to_bytes_into(&mut data)
                .context(ValueEncode { node_type })?;

            data_buf
                .write_u32::<BigEndian>(total_size as u32)
                .context(NodeSize {
                    node_type,
                    size: total_size as u32,
                })?;
            data_buf.write_all(&data).context(DataWrite { node_type })?;
            data_buf
                .realign_writes(None)
                .context(DataBuffer { node_type })?;
        },
        value => {
            if is_array {
                panic!("Attempted to write non-array value but was marked as array");
            }

            let data = value.to_bytes().context(ValueEncode { node_type })?;
            data_buf
                .write_aligned(node_type, &data)
                .context(DataBuffer { node_type })?;
        },
    };

    Ok(())
}

pub trait Writeable {
    fn write_node(
        &self,
        options: &Options,
        node_buf: &mut ByteBufferWrite,
        data_buf: &mut ByteBufferWrite,
    ) -> Result<(), WriterError>;
}

impl Writeable for NodeCollection {
    fn write_node(
        &self,
        options: &Options,
        node_buf: &mut ByteBufferWrite,
        data_buf: &mut ByteBufferWrite,
    ) -> Result<(), WriterError> {
        let (node_type, is_array) = self.base().node_type_tuple();
        let array_mask = if is_array { ARRAY_MASK } else { 0 };
        let name = self
            .base()
            .key()
            .context(DefinitionValue { node_type })?
            .ok_or(WriterError::NoNodeKey)?;

        debug!("NodeCollection write_node => name: {}, type: {:?}, type_size: {}, type_count: {}, is_array: {}",
            name,
            node_type,
            node_type.size,
            node_type.count,
            is_array);

        node_buf
            .write_u8(node_type as u8 | array_mask)
            .context(DataWrite { node_type })?;

        match options.compression {
            CompressionType::Compressed => {
                Sixbit::pack(&mut **node_buf, &name).context(NodeSixbitName)?
            },
            CompressionType::Uncompressed => {
                let data =
                    options
                        .encoding
                        .encode_bytes(&name)
                        .context(NodeUncompressedNameEncode {
                            encoding: options.encoding,
                        })?;
                let len = (data.len() - 1) as u8;
                node_buf
                    .write_u8(len | ARRAY_MASK)
                    .context(NodeUncompressedNameLength)?;
                node_buf
                    .write_all(&data)
                    .context(NodeUncompressedNameData)?;
            },
        };

        if node_type != StandardType::NodeStart {
            let value = self.base().value().context(DefinitionValue { node_type })?;
            write_value(options, data_buf, node_type, is_array, &value)?;
        }

        for attr in self.attributes() {
            let node_type = StandardType::Attribute;
            let key = attr
                .key()
                .context(DefinitionKey { node_type })?
                .ok_or(WriterError::NoNodeKey)?;
            let value = attr.value_bytes().ok_or(WriterError::NoNodeValue)?;

            trace!(
                "NodeCollection write_node => attr: {}, value: 0x{:02x?}",
                key,
                value
            );

            data_buf
                .buf_write(value)
                .context(DataBuffer { node_type })?;

            node_buf
                .write_u8(StandardType::Attribute as u8)
                .context(DataWrite { node_type })?;

            match options.compression {
                CompressionType::Compressed => {
                    Sixbit::pack(&mut **node_buf, &key).context(NodeSixbitName)?
                },
                CompressionType::Uncompressed => {
                    let data = options.encoding.encode_bytes(&key).context(
                        NodeUncompressedNameEncode {
                            encoding: options.encoding,
                        },
                    )?;
                    let len = (data.len() - 1) as u8;
                    node_buf
                        .write_u8(len | ARRAY_MASK)
                        .context(NodeUncompressedNameLength)?;
                    node_buf
                        .write_all(&data)
                        .context(NodeUncompressedNameData)?;
                },
            };
        }

        for child in self.children() {
            child.write_node(options, node_buf, data_buf)?;
        }

        // node end always has the array bit set
        node_buf
            .write_u8(StandardType::NodeEnd as u8 | ARRAY_MASK)
            .context(NodeType {
                node_type: StandardType::NodeEnd,
            })?;

        Ok(())
    }
}

impl Writeable for Node {
    fn write_node(
        &self,
        options: &Options,
        node_buf: &mut ByteBufferWrite,
        data_buf: &mut ByteBufferWrite,
    ) -> Result<(), WriterError> {
        let (node_type, is_array) = match self.value() {
            Some(Value::Array(ref values)) => (values.standard_type(), true),
            Some(ref value) => (value.standard_type(), false),
            None => (StandardType::NodeStart, false),
        };
        let array_mask = if is_array { ARRAY_MASK } else { 0 };

        debug!(
            "Node::write_node => name: {}, type: {:?}, type_size: {}, type_count: {}, is_array: {}",
            self.key(),
            node_type,
            node_type.size,
            node_type.count,
            is_array
        );

        node_buf
            .write_u8(node_type as u8 | array_mask)
            .context(DataWrite {
                node_type: node_type,
            })?;
        match options.compression {
            CompressionType::Compressed => {
                Sixbit::pack(&mut **node_buf, &self.key()).context(NodeSixbitName)?
            },
            CompressionType::Uncompressed => {
                let data = options.encoding.encode_bytes(&self.key()).context(
                    NodeUncompressedNameEncode {
                        encoding: options.encoding,
                    },
                )?;
                let len = (data.len() - 1) as u8;
                node_buf
                    .write_u8(len | ARRAY_MASK)
                    .context(NodeUncompressedNameLength)?;
                node_buf
                    .write_all(&data)
                    .context(NodeUncompressedNameData)?;
            },
        };

        if let Some(value) = self.value() {
            write_value(options, data_buf, node_type, is_array, value)?;
        }

        if let Some(attributes) = self.attributes() {
            for (key, value) in attributes {
                trace!("Node write_node => attr: {}, value: {}", key, value);

                data_buf
                    .write_str(options.encoding, value)
                    .context(DataBuffer { node_type })?;

                node_buf
                    .write_u8(StandardType::Attribute as u8)
                    .context(DataWrite {
                        node_type: StandardType::Attribute,
                    })?;

                match options.compression {
                    CompressionType::Compressed => {
                        Sixbit::pack(&mut **node_buf, &key).context(NodeSixbitName)?
                    },
                    CompressionType::Uncompressed => {
                        let data = options.encoding.encode_bytes(&key).context(
                            NodeUncompressedNameEncode {
                                encoding: options.encoding,
                            },
                        )?;
                        let len = (data.len() - 1) as u8;
                        node_buf
                            .write_u8(len | ARRAY_MASK)
                            .context(NodeUncompressedNameLength)?;
                        node_buf
                            .write_all(&data)
                            .context(NodeUncompressedNameData)?;
                    },
                };
            }
        }

        if let Some(children) = self.children() {
            for child in children {
                child.write_node(options, node_buf, data_buf)?;
            }
        }

        // node end always has the array bit set
        node_buf
            .write_u8(StandardType::NodeEnd as u8 | ARRAY_MASK)
            .context(NodeType {
                node_type: StandardType::NodeEnd,
            })?;

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
        Self { options }
    }

    pub fn to_binary<T>(&mut self, input: &T) -> Result<Vec<u8>, WriterError>
    where
        T: Writeable,
    {
        let mut header = Cursor::new(Vec::with_capacity(8));
        header.write_u8(SIGNATURE).context(Signature)?;

        let compression = self.options.compression.to_byte();
        header.write_u8(compression).context(Compression)?;

        let encoding = self.options.encoding.to_byte();
        header.write_u8(encoding).context(Encoding)?;
        header.write_u8(0xFF ^ encoding).context(EncodingNegate)?;

        let mut node_buf = ByteBufferWrite::new(Vec::new());
        let mut data_buf = ByteBufferWrite::new(Vec::new());

        input.write_node(&self.options, &mut node_buf, &mut data_buf)?;

        node_buf
            .write_u8(StandardType::FileEnd as u8 | ARRAY_MASK)
            .context(NodeType {
                node_type: StandardType::FileEnd,
            })?;
        node_buf.realign_writes(None).context(NodeBuffer {
            node_type: StandardType::FileEnd,
        })?;

        let mut output = header.into_inner();

        let node_buf = node_buf.into_inner();
        debug!(
            "to_binary_internal => node_buf len: {0} (0x{0:x})",
            node_buf.len()
        );
        output
            .write_u32::<BigEndian>(node_buf.len() as u32)
            .context(NodeBufferLength)?;
        output.extend_from_slice(&node_buf);

        let data_buf = data_buf.into_inner();
        debug!(
            "to_binary_internal => data_buf len: {0} (0x{0:x})",
            data_buf.len()
        );
        output
            .write_u32::<BigEndian>(data_buf.len() as u32)
            .context(DataBufferLength)?;
        output.extend_from_slice(&data_buf);

        Ok(output)
    }
}
