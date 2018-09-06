use std::io::{Seek, SeekFrom};

use byteorder::{BigEndian, ReadBytesExt};
use failure::ResultExt;

use byte_buffer::ByteBufferRead;
use compression::Compression;
use encoding_type::EncodingType;
use error::{KbinErrorKind, Result};
use node::{Key, NodeData, NodeDefinition};
use node_types::StandardType;
use sixbit::Sixbit;

use super::{ARRAY_MASK, SIGNATURE};

pub struct Reader<'buf> {
  compression: Compression,
  encoding: EncodingType,

  pub(crate) node_buf: ByteBufferRead<'buf>,
  pub(crate) data_buf: ByteBufferRead<'buf>,

  data_buf_start: u64,

  last_node_type: Option<(StandardType, bool)>,
  last_node_identifier: Option<String>,
}

impl<'buf> Reader<'buf> {
  pub fn new(input: &'buf [u8]) -> Result<Self> {
    // Node buffer starts from the beginning.
    // Data buffer starts later after reading `len_data`.
    let mut node_buf = ByteBufferRead::new(&input[..]);

    let signature = node_buf.read_u8().context(KbinErrorKind::HeaderRead("signature"))?;
    if signature != SIGNATURE {
      return Err(KbinErrorKind::HeaderValue("signature").into());
    }

    let compress_byte = node_buf.read_u8().context(KbinErrorKind::HeaderRead("compression"))?;
    let compression = Compression::from_byte(compress_byte)?;

    let encoding_byte = node_buf.read_u8().context(KbinErrorKind::HeaderRead("encoding"))?;
    let encoding_negation = node_buf.read_u8().context(KbinErrorKind::HeaderRead("encoding negation"))?;
    let encoding = EncodingType::from_byte(encoding_byte)?;
    if encoding_negation != !encoding_byte {
      return Err(KbinErrorKind::HeaderValue("encoding negation").into());
    }

    info!("signature: 0x{:X}, compression: 0x{:X} ({:?}), encoding: 0x{:X} ({:?})", signature, compress_byte, compression, encoding_byte, encoding);

    let len_node = node_buf.read_u32::<BigEndian>().context(KbinErrorKind::LenNodeRead)?;
    info!("len_node: {0} (0x{0:x})", len_node);

    // We have read 8 bytes so far, so offset the start of the data buffer from
    // the start of the input data.
    let data_buf_start = len_node + 8;
    let mut data_buf = ByteBufferRead::new(&input[(data_buf_start as usize)..]);

    let len_data = data_buf.read_u32::<BigEndian>().context(KbinErrorKind::LenDataRead)?;
    info!("len_data: {0} (0x{0:x})", len_data);

    Ok(Self {
      compression,
      encoding,

      node_buf,
      data_buf,

      data_buf_start: data_buf_start as u64,

      last_node_type: None,
      last_node_identifier: None,
    })
  }

  fn parse_node_type(raw_node_type: u8) -> Result<(StandardType, bool)> {
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

  #[inline]
  pub fn last_node_type(&self) -> Option<(StandardType, bool)> {
    self.last_node_type
  }

  #[inline]
  pub fn last_identifier(&self) -> Option<&str> {
    self.last_node_identifier.as_ref().map(String::as_str)
  }

  pub fn check_if_node_buffer_end(&self) -> Result<()> {
    if self.node_buf.position() >= self.data_buf_start {
      Err(KbinErrorKind::EndOfNodeBuffer.into())
    } else {
      Ok(())
    }
  }

  pub fn peek_node_type(&self) -> Result<(StandardType, bool)> {
    self.check_if_node_buffer_end()?;

    let pos = self.node_buf.position();
    let raw_node_type = self.node_buf.get_ref()[pos as usize];
    Self::parse_node_type(raw_node_type)
  }

  pub fn peek_node_identifier(&mut self) -> Result<String> {
    self.check_if_node_buffer_end()?;

    let old_pos = self.node_buf.position();
    let _raw_node_type = self.node_buf.read_u8().context(KbinErrorKind::NodeTypeRead)?;
    let value = match self.compression {
      Compression::Compressed => {
        let size = Sixbit::size(&mut *self.node_buf)?;
        let data = self.node_buf.get(size.real_len as u32)?;
        Sixbit::unpack(data, size)?
      },
      Compression::Uncompressed => {
        let length = (self.node_buf.read_u8().context(KbinErrorKind::DataRead(1))? & !ARRAY_MASK) + 1;
        let bytes = self.node_buf.get(length as u32)?;
        self.encoding.decode_bytes(bytes)?
      },
    };

    let size = self.node_buf.position() - old_pos;
    self.node_buf.seek(SeekFrom::Start(old_pos)).context(KbinErrorKind::DataRead(size as usize))?;

    Ok(value)
  }

  pub fn read_node_type(&mut self) -> Result<(StandardType, bool)> {
    self.check_if_node_buffer_end()?;

    let raw_node_type = self.node_buf.read_u8().context(KbinErrorKind::NodeTypeRead)?;
    let value = Self::parse_node_type(raw_node_type)?;
    self.last_node_type = Some(value);

    Ok(value)
  }

  pub fn read_node_identifier(&mut self) -> Result<String> {
    let value = match self.compression {
      Compression::Compressed => {
        let size = Sixbit::size(&mut *self.node_buf)?;
        let data = self.node_buf.get(size.real_len as u32)?;
        Sixbit::unpack(data, size)?
      },
      Compression::Uncompressed => {
        let length = (self.node_buf.read_u8().context(KbinErrorKind::DataRead(1))? & !ARRAY_MASK) + 1;
        let data = self.node_buf.get(length as u32)?;
        self.encoding.decode_bytes(data)?
      },
    };
    debug!("Reader::read_node_identifier() => value: {:?}", value);

    self.last_node_identifier = Some(value.clone());

    Ok(value)
  }

  pub fn read_node_data(&mut self, node_type: (StandardType, bool)) -> Result<&'buf [u8]> {
    let (node_type, is_array) = node_type;
    trace!("Reader::read_node_data(node_type: {:?}, is_array: {})", node_type, is_array);

    let value = match node_type {
      StandardType::Attribute |
      StandardType::String => self.data_buf.buf_read()?,
      StandardType::Binary => self.read_bytes()?,

      _ if is_array => {
        let arr_size = self.read_u32().context(KbinErrorKind::ArrayLengthRead)?;
        let data = self.data_buf.get(arr_size)?;
        self.data_buf.realign_reads(None)?;

        data
      },
      node_type => self.data_buf.get_aligned(*node_type)?,
    };
    debug!("Reader::read_node_data(node_type: {:?}, is_array: {}) => value: 0x{:02x?}", node_type, is_array, value);

    Ok(value)
  }

  pub fn read_node_definition(&mut self) -> Result<NodeDefinition<'buf>> {
    let node_type = self.read_node_type()?;
    match node_type.0 {
      StandardType::NodeEnd |
      StandardType::FileEnd => {
        Ok(NodeDefinition::new(self.encoding, node_type))
      }
      _ => {
        let key = match self.compression {
          Compression::Compressed => {
            let size = Sixbit::size(&mut *self.node_buf)?;
            let data = self.node_buf.get(size.real_len as u32)?;
            Key::Compressed { size, data }
          },
          Compression::Uncompressed => {
            let encoding = self.encoding;
            let length = (self.node_buf.read_u8().context(KbinErrorKind::DataRead(1))? & !ARRAY_MASK) + 1;
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

  pub fn read_string(&mut self) -> Result<String> {
    let value = self.data_buf.read_str(self.encoding)?;
    debug!("Reader::read_string() => value: {:?}", value);

    Ok(value)
  }

  pub fn read_u8(&mut self) -> Result<u8> {
    let value = self.data_buf.read_u8().context(KbinErrorKind::DataReadOneByte)?;
    debug!("Reader::read_u8() => value: {}", value);

    Ok(value)
  }

  pub fn read_u32(&mut self) -> Result<u32> {
    let value = self.data_buf.read_u32::<BigEndian>().context(KbinErrorKind::DataRead(4))?;
    debug!("Reader::read_u32() => result: {}", value);

    Ok(value)
  }

  #[inline]
  pub fn read_bytes(&mut self) -> Result<&'buf [u8]> {
    self.data_buf.buf_read()
  }
}
