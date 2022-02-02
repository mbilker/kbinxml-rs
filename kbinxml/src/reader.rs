use std::io::{self, Cursor, Seek, SeekFrom};

use byteorder::{BigEndian, ReadBytesExt};
use bytes::Bytes;
use snafu::{ResultExt, Snafu};

use crate::byte_buffer::{ByteBufferError, ByteBufferRead};
use crate::compression_type::{CompressionType, UnknownCompression};
use crate::encoding_type::{EncodingError, EncodingType};
use crate::node::{Key, NodeData, NodeDefinition};
use crate::node_types::{StandardType, UnknownKbinType};
use crate::sixbit::{Sixbit, SixbitError};
use crate::{ARRAY_MASK, SIGNATURE};

#[derive(Debug, Snafu)]
pub enum ReaderError {
    #[snafu(display("Failed to read signature from header"))]
    Signature { source: io::Error },

    #[snafu(display("Invalid signature read from header (signature: 0x{:x})", signature))]
    InvalidSignature { signature: u8 },

    #[snafu(display("Failed to read compression type from header"))]
    Compression { source: io::Error },

    #[snafu(display("Invalid compression type read from header"))]
    InvalidCompression { source: UnknownCompression },

    #[snafu(display("Failed to read encoding type from header"))]
    Encoding { source: io::Error },

    #[snafu(display("Failed to read encoding type inverted value from header"))]
    EncodingNegate { source: io::Error },

    #[snafu(display("Invalid encoding type read from header"))]
    InvalidEncoding { source: EncodingError },

    #[snafu(display("Mismatched encoding type and encoding type inverted values from header"))]
    MismatchedEncoding,

    #[snafu(display("Failed to read node buffer length"))]
    NodeBufferLength { source: io::Error },

    #[snafu(display("Failed to read data buffer length"))]
    DataBufferLength { source: io::Error },

    #[snafu(display(
        "Failed to seek forward {} bytes in input buffer for data buffer length",
        len_node
    ))]
    DataLengthSeek { len_node: u32, source: io::Error },

    #[snafu(display("Attempted to read past the end of the node buffer"))]
    EndOfNodeBuffer,

    #[snafu(display("Failed to read node type"))]
    NodeType { source: io::Error },

    #[snafu(display("Invalid node type read"))]
    InvalidNodeType { source: UnknownKbinType },

    #[snafu(display("Failed to read sixbit node name"))]
    NodeSixbitName { source: SixbitError },

    #[snafu(display("Failed to read array node length"))]
    ArrayLength { source: io::Error },

    #[snafu(display("Failed to read node name length"))]
    NameLength { source: io::Error },

    #[snafu(display("Failed to read {} bytes from data buffer", size))]
    DataRead { size: usize, source: io::Error },

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
}

pub struct Reader {
    compression: CompressionType,
    encoding: EncodingType,

    pub(crate) node_buf: ByteBufferRead,
    pub(crate) data_buf: ByteBufferRead,

    data_buf_start: u64,
}

impl Reader {
    pub fn new(input: Bytes) -> Result<Self, ReaderError> {
        let mut header = Cursor::new(&input);

        let signature = header.read_u8().context(SignatureSnafu)?;
        if signature != SIGNATURE {
            return Err(ReaderError::InvalidSignature { signature });
        }

        let compress_byte = header.read_u8().context(CompressionSnafu)?;
        let compression =
            CompressionType::from_byte(compress_byte).context(InvalidCompressionSnafu)?;

        let encoding_byte = header.read_u8().context(EncodingSnafu)?;
        let encoding_negation = header.read_u8().context(EncodingNegateSnafu)?;
        let encoding = EncodingType::from_byte(encoding_byte).context(InvalidEncodingSnafu)?;
        if encoding_negation != !encoding_byte {
            return Err(ReaderError::MismatchedEncoding);
        }

        info!(
            "signature: 0x{:X}, compression: 0x{:X} ({:?}), encoding: 0x{:X} ({:?})",
            signature, compress_byte, compression, encoding_byte, encoding
        );

        let len_node = header
            .read_u32::<BigEndian>()
            .context(NodeBufferLengthSnafu)?;
        info!("len_node: {0} (0x{0:x})", len_node);

        // The length of the data buffer is the 4 bytes right after the node buffer.
        header
            .seek(SeekFrom::Current(len_node as i64))
            .context(DataLengthSeekSnafu { len_node })?;

        let len_data = header
            .read_u32::<BigEndian>()
            .context(DataBufferLengthSnafu)?;
        info!("len_data: {0} (0x{0:x})", len_data);

        // We have read 8 bytes so far, so offset the start of the node buffer from
        // the start of the input data. After that is the length of the data buffer.
        // The data buffer is everything after that.
        let node_buffer_end = 8 + len_node as usize;
        let data_buffer_start = node_buffer_end + 4;
        let node_buf = ByteBufferRead::new(input.slice(8..node_buffer_end));
        let data_buf = ByteBufferRead::new(input.slice(data_buffer_start..));

        Ok(Self {
            compression,
            encoding,

            node_buf,
            data_buf,

            data_buf_start: data_buffer_start as u64,
        })
    }

    fn parse_node_type(raw_node_type: u8) -> Result<(StandardType, bool), ReaderError> {
        let is_array = raw_node_type & ARRAY_MASK == ARRAY_MASK;
        let node_type = raw_node_type & !ARRAY_MASK;

        let xml_type = StandardType::from_u8(node_type).context(InvalidNodeTypeSnafu)?;
        debug!(
            "Reader::parse_node_type() => raw_node_type: {}, node_type: {:?} ({}), is_array: {}",
            raw_node_type, xml_type, node_type, is_array
        );

        Ok((xml_type, is_array))
    }

    #[inline]
    pub fn encoding(&self) -> EncodingType {
        self.encoding
    }

    pub fn check_if_node_buffer_end(&self) -> Result<(), ReaderError> {
        if self.node_buf.position() >= self.data_buf_start {
            Err(ReaderError::EndOfNodeBuffer)
        } else {
            Ok(())
        }
    }

    pub fn read_node_type(&mut self) -> Result<(StandardType, bool), ReaderError> {
        self.check_if_node_buffer_end()?;

        let raw_node_type = self.node_buf.read_u8().context(NodeTypeSnafu)?;
        let value = Self::parse_node_type(raw_node_type)?;

        Ok(value)
    }

    pub fn read_node_data(
        &mut self,
        node_type: StandardType,
        is_array: bool,
    ) -> Result<Bytes, ReaderError> {
        trace!(
            "Reader::read_node_data(node_type: {:?}, is_array: {})",
            node_type,
            is_array
        );

        let value = match node_type {
            StandardType::Attribute | StandardType::String => self
                .data_buf
                .buf_read()
                .context(DataBufferSnafu { node_type })?,
            StandardType::Binary => self.read_bytes().context(DataBufferSnafu { node_type })?,
            StandardType::NodeStart | StandardType::NodeEnd | StandardType::FileEnd => Bytes::new(),
            node_type if is_array => {
                let arr_size = self
                    .data_buf
                    .read_u32::<BigEndian>()
                    .context(ArrayLengthSnafu)?;
                let data = self
                    .data_buf
                    .get(arr_size)
                    .context(DataBufferSnafu { node_type })?;
                self.data_buf
                    .realign_reads(None)
                    .context(DataBufferSnafu { node_type })?;

                data
            },
            node_type => self
                .data_buf
                .get_aligned(node_type)
                .context(DataBufferSnafu { node_type })?,
        };
        debug!(
            "Reader::read_node_data(node_type: {:?}, is_array: {}) => value: 0x{:02x?}",
            node_type,
            is_array,
            &value[..]
        );

        Ok(value)
    }

    pub fn read_node_definition(&mut self) -> Result<NodeDefinition, ReaderError> {
        let (node_type, is_array) = self.read_node_type()?;

        match node_type {
            StandardType::NodeEnd | StandardType::FileEnd => {
                Ok(NodeDefinition::new(self.encoding, node_type, is_array))
            },
            _ => {
                let key = match self.compression {
                    CompressionType::Compressed => {
                        let size =
                            Sixbit::size(&mut *self.node_buf).context(NodeSixbitNameSnafu)?;
                        let data = self
                            .node_buf
                            .get(size.real_len as u32)
                            .context(NodeBufferSnafu { node_type })?;

                        Key::Compressed { size, data }
                    },
                    CompressionType::Uncompressed => {
                        let encoding = self.encoding;
                        let length =
                            (self.node_buf.read_u8().context(NameLengthSnafu)? & !ARRAY_MASK) + 1;
                        let data = self
                            .node_buf
                            .get(length as u32)
                            .context(NodeBufferSnafu { node_type })?;

                        Key::Uncompressed { encoding, data }
                    },
                };
                let value_data = self.read_node_data(node_type, is_array)?;

                Ok(NodeDefinition::with_data(
                    self.encoding,
                    node_type,
                    is_array,
                    NodeData::Some { key, value_data },
                ))
            },
        }
    }

    pub fn read_u32(&mut self) -> Result<u32, ReaderError> {
        let value = self
            .data_buf
            .read_u32::<BigEndian>()
            .context(DataReadSnafu { size: 4usize })?;
        debug!("Reader::read_u32() => result: {}", value);

        Ok(value)
    }

    #[inline]
    pub fn read_bytes(&mut self) -> Result<Bytes, ByteBufferError> {
        self.data_buf.buf_read()
    }
}

impl Iterator for Reader {
    type Item = NodeDefinition;

    fn next(&mut self) -> Option<NodeDefinition> {
        match self.read_node_definition() {
            Ok(v) => Some(v),
            Err(e) => {
                error!("Error reading node definition in `next()`: {}", e);
                None
            },
        }
    }
}
