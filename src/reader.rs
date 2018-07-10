use byteorder::{BigEndian, ReadBytesExt};
use failure::ResultExt;

use byte_buffer::ByteBufferRead;
use compression::Compression;
use encoding_type::EncodingType;
use error::{KbinErrorKind, Result};
use node_types::StandardType;
use sixbit::unpack_sixbit;
use super::{ARRAY_MASK, SIGNATURE, SIG_COMPRESSED};

pub struct Reader<'buf> {
  encoding: EncodingType,

  pub(crate) node_buf: ByteBufferRead<&'buf [u8]>,
  pub(crate) data_buf: ByteBufferRead<&'buf [u8]>,

  data_buf_start: u64,
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

    // TODO: support uncompressed
    let compress_byte = node_buf.read_u8().context(KbinErrorKind::HeaderRead("compression"))?;
    if compress_byte != SIG_COMPRESSED {
      return Err(KbinErrorKind::HeaderValue("compression").into());
    }

    let compressed = Compression::from_byte(compress_byte)?;

    let encoding_byte = node_buf.read_u8().context(KbinErrorKind::HeaderRead("encoding"))?;
    let encoding_negation = node_buf.read_u8().context(KbinErrorKind::HeaderRead("encoding negation"))?;
    let encoding = EncodingType::from_byte(encoding_byte)?;
    if encoding_negation != !encoding_byte {
      return Err(KbinErrorKind::HeaderValue("encoding negation").into());
    }

    info!("signature: 0x{:X}, compression: 0x{:X} ({:?}), encoding: 0x{:X} ({:?})", signature, compress_byte, compressed, encoding_byte, encoding);

    let len_node = node_buf.read_u32::<BigEndian>().context(KbinErrorKind::LenNodeRead)?;
    info!("len_node: {0} (0x{0:x})", len_node);

    // We have read 8 bytes so far, so offset the start of the data buffer from
    // the start of the input data.
    let data_buf_start = len_node + 8;
    let mut data_buf = ByteBufferRead::new(&input[(data_buf_start as usize)..]);

    let len_data = data_buf.read_u32::<BigEndian>().context(KbinErrorKind::LenDataRead)?;
    info!("len_data: {0} (0x{0:x})", len_data);

    //let node_buf_end = data_buf_start.into();

    Ok(Self {
      encoding,
      //read_mode: ReadMode::Single,
      //first_struct: true,
      //node_buf_end,
      node_buf,
      data_buf,

      data_buf_start: data_buf_start as u64,
    })
  }

  #[inline]
  pub fn encoding(&self) -> EncodingType {
    self.encoding
  }

  #[inline]
  pub fn data_buf_start(&self) -> u64 {
    self.data_buf_start
  }

  pub fn read_node_type(&mut self) -> Result<(StandardType, bool)> {
    let raw_node_type = self.node_buf.read_u8().context(KbinErrorKind::NodeTypeRead)?;
    let is_array = raw_node_type & ARRAY_MASK == ARRAY_MASK;
    let node_type = raw_node_type & !ARRAY_MASK;

    let xml_type = StandardType::from_u8(node_type);
    debug!("Reader::read_node_type() => raw_node_type: {}, node_type: {:?} ({}), is_array: {}",
      raw_node_type,
      xml_type,
      node_type,
      is_array);

    Ok((xml_type, is_array))
  }

  pub fn read_node_identifier(&mut self) -> Result<String> {
    let value = unpack_sixbit(&mut *self.node_buf)?;
    debug!("Reader::read_node_identifier() => value: {:?}", value);

    Ok(value)
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

  // TODO: make a more intelligent reader to avoid allocating the Vec
  #[inline]
  pub fn read_bytes(&mut self) -> Result<Vec<u8>> {
    self.data_buf.buf_read()
  }
}
