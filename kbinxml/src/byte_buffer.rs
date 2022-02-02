use std::cmp::max;
use std::io::{self, Cursor, Seek, SeekFrom, Write};
use std::ops::{Deref, DerefMut};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;
use snafu::{ResultExt, Snafu};

use crate::encoding_type::{EncodingError, EncodingType};
use crate::node_types::StandardType;

#[derive(Debug, Snafu)]
pub enum ByteBufferError {
    #[snafu(display(
        "Out-of-bounds read attempted at offset: {} with size: {}",
        offset,
        size
    ))]
    OutOfBounds { offset: usize, size: usize },

    #[snafu(display("Failed to read {} byte(s) from data buffer", size))]
    DataRead { size: usize, source: io::Error },

    #[snafu(display("Failed to read aligned {} byte(s) from data buffer", size))]
    ReadAligned {
        size: usize,
        source: Box<ByteBufferError>,
    },

    #[snafu(display("Failed to read data size from data buffer"))]
    ReadSize { source: io::Error },

    #[snafu(display(
        "Failed to seek forward {} byte(s) in data buffer after size read",
        size
    ))]
    ReadSizeSeek { size: usize, source: io::Error },

    #[snafu(display("Failed to write length to data buffer (len: {})", len))]
    WriteLength { len: usize, source: io::Error },

    #[snafu(display("Failed to write data byte {} to data buffer", offset))]
    WriteDataByte { offset: usize, source: io::Error },

    #[snafu(display("Failed to write data block to data buffer"))]
    WriteDataBlock { source: io::Error },

    #[snafu(display("Failed to encode string"))]
    StringEncode { source: EncodingError },

    #[snafu(display("Failed to write padding {} byte(s) to data buffer", size))]
    WritePadding { size: usize, source: io::Error },

    #[snafu(display(
        "Mismatched size for {} node data (expected: {}, actual: {})",
        node_type,
        expected,
        actual
    ))]
    WriteSizeMismatch {
        node_type: StandardType,
        expected: usize,
        actual: usize,
    },

    #[snafu(display("Failed to seek to {} in data buffer", offset))]
    SeekOffset { offset: usize, source: io::Error },

    #[snafu(display("Failed to seek forward {} byte(s) in data buffer", size))]
    SeekForward { size: usize, source: io::Error },
}

/// Remove trailing null bytes, used for the `String` type
pub(crate) fn strip_trailing_null_bytes(data: &[u8]) -> &[u8] {
    let len = data.len();

    if len == 0 {
        return data;
    }

    let mut index = len - 1;
    while index > 0 && index < len && data[index] == 0x00 {
        index -= 1;
    }

    // Handle case where the buffer is only a null byte
    if index == 0 && data.len() == 1 && data[index] == 0x00 {
        &[]
    } else {
        &data[..=index]
    }
}

pub struct ByteBufferRead {
    cursor: Cursor<Bytes>,
    buffer: Bytes,
    offset_1: usize,
    offset_2: usize,
}

pub struct ByteBufferWrite {
    buffer: Cursor<Vec<u8>>,
    offset_1: u64,
    offset_2: u64,
}

impl ByteBufferRead {
    pub fn new(buffer: Bytes) -> Self {
        Self {
            cursor: Cursor::new(buffer.clone()),
            buffer,
            offset_1: 0,
            offset_2: 0,
        }
    }

    #[inline]
    fn data_buf_offset(&self) -> usize {
        // Position is not the index of the previously read byte, it is the current
        // index (offset).
        //
        // This is so much fun to debug.
        //data_buf.position() - 1
        self.cursor.position() as usize
    }

    fn check_read_size(&self, start: usize, size: usize) -> Result<usize, ByteBufferError> {
        let end = start + size;
        if end > self.buffer.len() {
            Err(ByteBufferError::OutOfBounds {
                offset: start,
                size,
            })
        } else {
            Ok(end)
        }
    }

    fn buf_read_size(&mut self, size: usize) -> Result<Bytes, ByteBufferError> {
        // To avoid an allocation of a `Vec` here, the raw input byte array is used
        let start = self.data_buf_offset();
        let end = self.check_read_size(start, size)?;

        let data = self.buffer.slice(start..end);
        trace!(
            "buf_read_size => index: {}, size: {}, data: 0x{:02x?}",
            self.cursor.position(),
            data.len(),
            &*data
        );

        self.cursor
            .seek(SeekFrom::Current(size as i64))
            .context(ReadSizeSeekSnafu { size })?;

        Ok(data)
    }

    pub fn buf_read(&mut self) -> Result<Bytes, ByteBufferError> {
        let size = self.cursor.read_u32::<BigEndian>().context(ReadSizeSnafu)?;
        debug!(
            "buf_read => index: {}, size: {}",
            self.cursor.position(),
            size
        );

        let data = self.buf_read_size(size as usize)?;
        self.realign_reads(None)?;

        Ok(data)
    }

    pub fn get(&mut self, size: u32) -> Result<Bytes, ByteBufferError> {
        let data = self.buf_read_size(size as usize)?;
        trace!("get => size: {}, data: 0x{:02x?}", size, &*data);

        Ok(data)
    }

    pub fn get_aligned(&mut self, node_type: StandardType) -> Result<Bytes, ByteBufferError> {
        if self.offset_1 % 4 == 0 {
            self.offset_1 = self.data_buf_offset();
        }
        if self.offset_2 % 4 == 0 {
            self.offset_2 = self.data_buf_offset();
        }

        let old_pos = self.data_buf_offset();
        let size = node_type.size * node_type.count;
        trace!("get_aligned => old_pos: {}, size: {}", old_pos, size);

        let (check_old, data) = match size {
            1 => {
                let end = self.check_read_size(self.offset_1, 1)?;
                let data = self.buffer.slice(self.offset_1..end);
                self.offset_1 += 1;

                (true, data)
            },
            2 => {
                let end = self.check_read_size(self.offset_2, 2)?;
                let data = self.buffer.slice(self.offset_2..end);
                self.offset_2 += 2;

                (true, data)
            },
            size => {
                let data = self
                    .buf_read_size(size as usize)
                    .map_err(Box::new)
                    .context(ReadAlignedSnafu { size })?;
                self.realign_reads(None)?;

                (false, data)
            },
        };

        if check_old {
            let trailing = max(self.offset_1, self.offset_2);
            trace!(
                "get_aligned => old_pos: {}, trailing: {}",
                old_pos,
                trailing
            );

            if old_pos < trailing {
                self.cursor
                    .seek(SeekFrom::Start(trailing as u64))
                    .context(SeekOffsetSnafu { offset: trailing })?;
                self.realign_reads(None)?;
            }
        }

        Ok(data)
    }

    pub fn realign_reads(&mut self, size: Option<u64>) -> Result<(), ByteBufferError> {
        let size = size.unwrap_or(4);
        trace!(
            "realign_reads => position: {}, size: {}",
            self.cursor.position(),
            size
        );

        while self.cursor.position() % size > 0 {
            self.cursor
                .seek(SeekFrom::Current(1))
                .context(SeekForwardSnafu { size: 1usize })?;
        }
        trace!("realign_reads => realigned to: {}", self.cursor.position());

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

    pub fn buf_write(&mut self, data: &[u8]) -> Result<(), ByteBufferError> {
        self.buffer
            .write_u32::<BigEndian>(data.len() as u32)
            .context(WriteLengthSnafu { len: data.len() })?;
        debug!(
            "buf_write => index: {}, size: {}",
            self.buffer.position(),
            data.len()
        );

        self.buffer.write_all(data).context(WriteDataBlockSnafu)?;
        trace!(
            "buf_write => index: {}, size: {}, data: 0x{:02x?}",
            self.buffer.position(),
            data.len(),
            data
        );

        self.realign_writes(None)?;

        Ok(())
    }

    pub fn write_str(&mut self, encoding: EncodingType, data: &str) -> Result<(), ByteBufferError> {
        trace!(
            "write_str => input: {}, data: 0x{:02x?}",
            data,
            data.as_bytes()
        );

        let bytes = encoding.encode_bytes(data).context(StringEncodeSnafu)?;
        self.buf_write(&bytes)?;

        Ok(())
    }

    pub fn write_aligned(
        &mut self,
        node_type: StandardType,
        data: &[u8],
    ) -> Result<(), ByteBufferError> {
        if self.offset_1 % 4 == 0 {
            self.offset_1 = self.data_buf_offset();
        }
        if self.offset_2 % 4 == 0 {
            self.offset_2 = self.data_buf_offset();
        }

        let old_pos = self.data_buf_offset();
        let size = node_type.size * node_type.count;
        trace!(
            "write_aligned => old_pos: {}, size: {}, data: 0x{:02x?}",
            old_pos,
            size,
            data
        );

        if size != data.len() {
            return Err(ByteBufferError::WriteSizeMismatch {
                node_type,
                expected: size,
                actual: data.len(),
            });
        }

        let check_old = match size {
            1 => {
                // Make room for new DWORD
                if self.offset_1 % 4 == 0 {
                    self.buffer
                        .write_u32::<BigEndian>(0)
                        .context(WritePaddingSnafu { size: 4usize })?;
                }

                self.buffer
                    .seek(SeekFrom::Start(self.offset_1))
                    .context(SeekOffsetSnafu {
                        offset: self.offset_1 as usize,
                    })?;
                self.buffer
                    .write_u8(data[0])
                    .context(WriteDataByteSnafu { offset: 1usize })?;
                self.offset_1 += 1;

                true
            },
            2 => {
                // Make room for new DWORD
                if self.offset_2 % 4 == 0 {
                    self.buffer
                        .write_u32::<BigEndian>(0)
                        .context(WritePaddingSnafu { size: 4usize })?;
                }

                self.buffer
                    .seek(SeekFrom::Start(self.offset_2))
                    .context(SeekOffsetSnafu {
                        offset: self.offset_2 as usize,
                    })?;
                self.buffer
                    .write_u8(data[0])
                    .context(WriteDataByteSnafu { offset: 1usize })?;
                self.buffer
                    .write_u8(data[1])
                    .context(WriteDataByteSnafu { offset: 2usize })?;
                self.offset_2 += 2;

                true
            },
            _ => {
                self.buffer.write_all(data).context(WriteDataBlockSnafu)?;
                self.realign_writes(None)?;

                false
            },
        };

        if check_old {
            self.buffer
                .seek(SeekFrom::Start(old_pos))
                .context(SeekOffsetSnafu {
                    offset: old_pos as usize,
                })?;

            let trailing = max(self.offset_1, self.offset_2);
            trace!(
                "write_aligned => old_pos: {}, trailing: {}",
                old_pos,
                trailing
            );
            if old_pos < trailing {
                self.buffer
                    .seek(SeekFrom::Start(trailing))
                    .context(SeekOffsetSnafu {
                        offset: trailing as usize,
                    })?;
                self.realign_writes(None)?;
            }
        }

        Ok(())
    }

    pub fn realign_writes(&mut self, size: Option<u64>) -> Result<(), ByteBufferError> {
        let size = size.unwrap_or(4);
        trace!(
            "realign_writes => position: {}, size: {}",
            self.buffer.position(),
            size
        );

        while self.buffer.position() % size > 0 {
            self.buffer
                .write_u8(0)
                .context(WritePaddingSnafu { size: 1usize })?;
        }

        trace!("realign_writes => realigned to: {}", self.buffer.position());

        Ok(())
    }
}

impl Deref for ByteBufferRead {
    type Target = Cursor<Bytes>;

    fn deref(&self) -> &Self::Target {
        &self.cursor
    }
}

impl DerefMut for ByteBufferRead {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cursor
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
