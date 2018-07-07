use std::cmp::max;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::ops::{Deref, DerefMut};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use failure::ResultExt;

use node_types::KbinType;

pub use encoding_type::EncodingType;
pub use error::{KbinError, KbinErrorKind, Result};

pub struct ByteBufferRead<R: AsRef<[u8]>> {
  buffer: Cursor<R>,
  offset_1: u64,
  offset_2: u64,
}

pub struct ByteBufferWrite {
  buffer: Cursor<Vec<u8>>,
  offset_1: u64,
  offset_2: u64,
}

impl<R: AsRef<[u8]>> ByteBufferRead<R> {
  pub fn new(buffer: R) -> Self {
    Self {
      buffer: Cursor::new(buffer),
      offset_1: 0,
      offset_2: 0,
    }
  }

  #[inline]
  fn data_buf_offset(&self) -> u64 {
    // Position is not the index of the previously read byte, it is the current
    // index (offset).
    //
    // This is so much fun to debug.
    //data_buf.position() - 1
    self.buffer.position()
  }

  pub fn buf_read(&mut self) -> Result<Vec<u8>> {
    let size = self.buffer.read_u32::<BigEndian>().context(KbinErrorKind::DataReadSize)?;
    debug!("data_buf_read => index: {}, size: {}", self.buffer.position(), size);

    let mut data = vec![0; size as usize];
    self.buffer.read_exact(&mut data).context(KbinErrorKind::DataRead(size as usize))?;
    trace!("data_buf_read => index: {}, size: {}, data: 0x{:02x?}", self.buffer.position(), data.len(), data);

    self.realign_reads(None)?;

    Ok(data)
  }

  pub fn read_str(&mut self, encoding: EncodingType) -> Result<String> {
    let mut data = self.buf_read()?;

    // Remove trailing null bytes
    let mut index = data.len() - 1;
    let len = data.len();
    while index > 0 && index < len && data[index] == 0x00 {
      index -= 1;
    }
    data.truncate(index + 1);
    trace!("data_buf_read_str => size: {}, data: 0x{:02x?}", data.len(), data);

    encoding.decode_bytes(data)
  }

  pub fn get(&mut self, size: u32) -> Result<Vec<u8>> {
    let mut data = vec![0; size as usize];
    self.buffer.read_exact(&mut data).context(KbinErrorKind::DataRead(size as usize))?;

    Ok(data)
  }

  pub fn get_aligned(&mut self, data_type: KbinType) -> Result<Vec<u8>> {
    if self.offset_1 % 4 == 0 {
      self.offset_1 = self.data_buf_offset();
    }
    if self.offset_2 % 4 == 0 {
      self.offset_2 = self.data_buf_offset();
    }

    let old_pos = self.data_buf_offset();
    let size = data_type.size * data_type.count;
    trace!("data_buf_get_aligned => old_pos: {}, size: {}", old_pos, size);

    let (check_old, data) = match size {
      1 => {
        self.buffer.seek(SeekFrom::Start(self.offset_1)).context(KbinErrorKind::Seek)?;

        let data = self.buffer.read_u8().context(KbinErrorKind::DataReadOneByte)?;
        self.offset_1 += 1;

        (true, vec![data])
      },
      2 => {
        self.buffer.seek(SeekFrom::Start(self.offset_2)).context(KbinErrorKind::Seek)?;

        let mut data = vec![0; 2];
        self.buffer.read_exact(&mut data).context(KbinErrorKind::DataReadTwoByte)?;
        self.offset_2 += 2;

        (true, data)
      },
      size => {
        let mut data = vec![0; size as usize];
        self.buffer.read_exact(&mut data).context(KbinErrorKind::DataReadAligned)?;
        self.realign_reads(None)?;

        (false, data)
      },
    };


    if check_old {
      self.buffer.seek(SeekFrom::Start(old_pos)).context(KbinErrorKind::Seek)?;

      let trailing = max(self.offset_1, self.offset_2);
      trace!("data_buf_get_aligned => old_pos: {}, trailing: {}", old_pos, trailing);
      if old_pos < trailing {
        self.buffer.seek(SeekFrom::Start(trailing)).context(KbinErrorKind::Seek)?;
        self.realign_reads(None)?;
      }
    }

    Ok(data)
  }

  pub fn realign_reads(&mut self, size: Option<u64>) -> Result<()> {
    let size = size.unwrap_or(4);
    trace!("data_buf_realign_reads => position: {}, size: {}", self.buffer.position(), size);

    while self.buffer.position() % size > 0 {
      self.buffer.seek(SeekFrom::Current(1)).context(KbinErrorKind::Seek)?;
    }
    trace!("data_buf_realign_reads => realigned to: {}", self.buffer.position());

    Ok(())
  }
}

impl ByteBufferWrite {
  pub fn new(buffer: Vec<u8>) -> Self {
    Self {
      buffer: Cursor::new(buffer),
      offset_1: 0,
      offset_2: 0,
    }
  }

  pub fn into_inner(self) -> Vec<u8> {
    self.buffer.into_inner()
  }

  #[inline]
  fn data_buf_offset(&self) -> u64 {
    // Position is not the index of the previously read byte, it is the current
    // index (offset).
    //
    // This is so much fun to debug.
    //data_buf.position() - 1
    self.buffer.position()
  }

  pub fn buf_write(&mut self, data: &[u8]) -> Result<()> {
    self.buffer.write_u32::<BigEndian>(data.len() as u32).context(KbinErrorKind::DataWrite("data length integer"))?;
    debug!("data_buf_write => index: {}, size: {}", self.buffer.position(), data.len());

    self.buffer.write_all(data).context(KbinErrorKind::DataWrite("data block"))?;
    trace!("data_buf_write => index: {}, size: {}, data: 0x{:02x?}", self.buffer.position(), data.len(), data);

    self.realign_writes(None)?;

    Ok(())
  }

  pub fn write_str(&mut self, encoding: EncodingType, data: &str) -> Result<()> {
    trace!("data_buf_write_str => input: {}, data: 0x{:02x?}", data, data.as_bytes());

    let bytes = encoding.encode_bytes(data)?;
    self.buf_write(&bytes)?;

    Ok(())
  }

  pub fn write_aligned(&mut self, data_type: KbinType, data: &[u8]) -> Result<()> {
    if self.offset_1 % 4 == 0 {
      self.offset_1 = self.data_buf_offset();
    }
    if self.offset_2 % 4 == 0 {
      self.offset_2 = self.data_buf_offset();
    }

    let old_pos = self.data_buf_offset();
    let size = (data_type.size as usize) * (data_type.count as usize);
    trace!("data_buf_write_aligned => old_pos: {}, size: {}", old_pos, size);

    if size != data.len() {
      return Err(KbinErrorKind::SizeMismatch(data_type, size, data.len()).into());
    }

    let check_old = match size {
      1 => {
        // Make room for new DWORD
        if self.offset_1 % 4 == 0 {
          self.buffer.write_u32::<BigEndian>(0).context(KbinErrorKind::DataWrite("empty DWORD"))?;
        }

        self.buffer.seek(SeekFrom::Start(self.offset_1)).context(KbinErrorKind::Seek)?;
        self.buffer.write_u8(data[0]).context(KbinErrorKind::DataWrite("1 byte value"))?;
        self.offset_1 += 1;

        true
      },
      2 => {
        // Make room for new DWORD
        if self.offset_2 % 4 == 0 {
          self.buffer.write_u32::<BigEndian>(0).context(KbinErrorKind::DataWrite("empty DWORD"))?;
        }

        self.buffer.seek(SeekFrom::Start(self.offset_2)).context(KbinErrorKind::Seek)?;
        self.buffer.write_u8(data[0]).context(KbinErrorKind::DataWrite("first byte of 2 byte value"))?;
        self.buffer.write_u8(data[1]).context(KbinErrorKind::DataWrite("second byte of 2 byte value"))?;
        self.offset_2 += 2;

        true
      },
      _ => {
        self.buffer.write_all(data).context(KbinErrorKind::DataWrite("large value"))?;
        self.realign_writes(None)?;

        false
      },
    };

    if check_old {
      self.buffer.seek(SeekFrom::Start(old_pos)).context(KbinErrorKind::Seek)?;

      let trailing = max(self.offset_1, self.offset_2);
      trace!("data_buf_write_aligned => old_pos: {}, trailing: {}", old_pos, trailing);
      if old_pos < trailing {
        self.buffer.seek(SeekFrom::Start(trailing)).context(KbinErrorKind::Seek)?;
        self.realign_writes(None)?;
      }
    }

    Ok(())
  }

  pub fn realign_writes(&mut self, size: Option<u64>) -> Result<()> {
    let size = size.unwrap_or(4);
    trace!("data_buf_realign_writes => position: {}, size: {}", self.buffer.position(), size);

    while self.buffer.position() % size > 0 {
      self.buffer.write_u8(0).context(KbinErrorKind::Seek)?;
    }
    trace!("data_buf_realign_writes => realigned to: {}", self.buffer.position());

    Ok(())
  }
}

impl<R> Deref for ByteBufferRead<R> where R: AsRef<[u8]> {
  type Target = Cursor<R>;

  fn deref(&self) -> &Self::Target {
    &self.buffer
  }
}

impl<R> DerefMut for ByteBufferRead<R> where R: AsRef<[u8]> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.buffer
  }
}

impl Deref for ByteBufferWrite {
  type Target = Cursor<Vec<u8>>;

  fn deref(&self) -> &Self::Target {
    &self.buffer
  }
}

impl DerefMut for ByteBufferWrite {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.buffer
  }
}
