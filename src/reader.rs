use std::io;

use bytes::Bytes;
use byteorder::{BigEndian, ReadBytesExt};
use snafu::{ResultExt, Snafu};

use crate::byte_buffer::{ByteBufferError, ByteBufferRead};
use crate::compression::Compression as CompressionType;
use crate::encoding_type::EncodingType;
use crate::error::KbinError;
use crate::node::{Key, NodeData, NodeDefinition};
use crate::node_types::StandardType;
use crate::sixbit::Sixbit;

use super::{ARRAY_MASK, SIGNATURE};

#[derive(Debug, Snafu)]
pub enum ReaderError {
  #[snafu(display("Failed to read signature from header"))]
  Signature {
    source: io::Error,
  },

  #[snafu(display("Failed to read compression type from header"))]
  Compression {
    source: io::Error,
  },

  #[snafu(display("Failed to read encoding type from header"))]
  Encoding {
    source: io::Error,
  },

  #[snafu(display("Failed to read encoding type inverted value from header"))]
  EncodingNegate {
    source: io::Error,
  },

  #[snafu(display("Failed to read node buffer length"))]
  NodeBufferLength {
    source: io::Error,
  },

  #[snafu(display("Failed to read data buffer length"))]
  DataBufferLength {
    source: io::Error,
  },

  #[snafu(display("Reached the end of the node buffer"))]
  EndOfNodeBuffer,

  #[snafu(display("Failed to read node type"))]
  NodeType {
    source: io::Error,
  },

  #[snafu(display("Failed to read array node length"))]
  ArrayLength {
    source: io::Error,
  },

  #[snafu(display("Failed to read node name length"))]
  NameLength {
    source: io::Error,
  },

  #[snafu(display("Failed to read {} bytes from data buffer", size))]
  DataRead {
    size: usize,
    source: io::Error,
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
  pub fn new(input: Bytes) -> Result<Self, KbinError> {
    // Node buffer starts from the beginning.
    // Data buffer starts later after reading `len_data`.
    let mut node_buf = ByteBufferRead::new(input.clone());

    let signature = node_buf.read_u8().context(Signature)?;
    if signature != SIGNATURE {
      return Err(KbinError::HeaderValue { field: "signature" });
    }

    let compress_byte = node_buf.read_u8().context(Compression)?;
    let compression = CompressionType::from_byte(compress_byte)?;

    let encoding_byte = node_buf.read_u8().context(Encoding)?;
    let encoding_negation = node_buf.read_u8().context(EncodingNegate)?;
    let encoding = EncodingType::from_byte(encoding_byte)?;
    if encoding_negation != !encoding_byte {
      return Err(KbinError::HeaderValue { field: "encoding negation" });
    }

    info!("signature: 0x{:X}, compression: 0x{:X} ({:?}), encoding: 0x{:X} ({:?})", signature, compress_byte, compression, encoding_byte, encoding);

    let len_node = node_buf.read_u32::<BigEndian>().context(NodeBufferLength)?;
    info!("len_node: {0} (0x{0:x})", len_node);

    // We have read 8 bytes so far, so offset the start of the data buffer from
    // the start of the input data.
    let data_buf_start = len_node + 8;
    let mut data_buf = ByteBufferRead::new(input.slice_from(data_buf_start as usize));

    let len_data = data_buf.read_u32::<BigEndian>().context(DataBufferLength)?;
    info!("len_data: {0} (0x{0:x})", len_data);

    Ok(Self {
      compression,
      encoding,

      node_buf,
      data_buf,

      data_buf_start: data_buf_start as u64,
    })
  }

  fn parse_node_type(raw_node_type: u8) -> Result<(StandardType, bool), KbinError> {
    let is_array = raw_node_type & ARRAY_MASK == ARRAY_MASK;
    let node_type = raw_node_type & !ARRAY_MASK;

    let xml_type = StandardType::from_u8(node_type);
    debug!("Reader::parse_node_type() => raw_node_type: {}, node_type: {:?} ({}), is_array: {}",
      raw_node_type,
      xml_type,
      node_type,
      is_array);

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

  pub fn read_node_type(&mut self) -> Result<(StandardType, bool), KbinError> {
    self.check_if_node_buffer_end()?;

    let raw_node_type = self.node_buf.read_u8().context(NodeType)?;
    let value = Self::parse_node_type(raw_node_type)?;

    Ok(value)
  }

  pub fn read_node_data(&mut self, node_type: (StandardType, bool)) -> Result<Bytes, KbinError> {
    let (node_type, is_array) = node_type;
    trace!("Reader::read_node_data(node_type: {:?}, is_array: {})", node_type, is_array);

    let value = match node_type {
      StandardType::Attribute |
      StandardType::String => self.data_buf.buf_read()?,
      StandardType::Binary => self.read_bytes()?,

      StandardType::NodeStart |
      StandardType::NodeEnd |
      StandardType::FileEnd => Bytes::new(),

      _ if is_array => {
        let arr_size = self.data_buf.read_u32::<BigEndian>().context(ArrayLength)?;
        let data = self.data_buf.get(arr_size)?;
        self.data_buf.realign_reads(None)?;

        data
      },
      node_type => self.data_buf.get_aligned(*node_type)?,
    };
    debug!("Reader::read_node_data(node_type: {:?}, is_array: {}) => value: 0x{:02x?}", node_type, is_array, value);

    Ok(value)
  }

  pub fn read_node_definition(&mut self) -> Result<NodeDefinition, KbinError> {
    let node_type = self.read_node_type()?;
    match node_type.0 {
      StandardType::NodeEnd |
      StandardType::FileEnd => {
        Ok(NodeDefinition::new(self.encoding, node_type))
      }
      _ => {
        let key = match self.compression {
          CompressionType::Compressed => {
            let size = Sixbit::size(&mut *self.node_buf)?;
            let data = self.node_buf.get(size.real_len as u32)?;
            Key::Compressed { size, data }
          },
          CompressionType::Uncompressed => {
            let encoding = self.encoding;
            let length = (self.node_buf.read_u8().context(NameLength)? & !ARRAY_MASK) + 1;
            let data = self.node_buf.get(length as u32)?;
            Key::Uncompressed { encoding, data }
          },
        };
        let value_data = self.read_node_data(node_type)?;
        let node_data = NodeData::Some { key, value_data };
        Ok(NodeDefinition::with_data(self.encoding, node_type, node_data))
      },
    }
  }

  pub fn read_u32(&mut self) -> Result<u32, ReaderError> {
    let value = self.data_buf.read_u32::<BigEndian>().context(DataRead { size: 4usize })?;
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
