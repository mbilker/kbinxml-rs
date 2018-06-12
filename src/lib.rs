#![feature(int_to_from_bytes)]

extern crate byteorder;
extern crate encoding;
extern crate minidom;
extern crate num;
extern crate rustc_hex;

#[macro_use] extern crate failure;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate log;

use std::cmp::max;
use std::fmt::Write as FmtWrite;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::result::Result as StdResult;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use failure::ResultExt;
use minidom::Element;
use rustc_hex::FromHex;

mod compression;
mod encoding_type;
mod error;
mod node_types;
mod options;
mod sixbit;

use compression::Compression;
use node_types::{KbinType, StandardType};
use sixbit::{pack_sixbit, unpack_sixbit};

// Public exports
pub use encoding_type::EncodingType;
pub use error::{KbinError, KbinErrorKind};
pub use options::EncodingOptions;

const SIGNATURE: u8 = 0xA0;

const SIG_COMPRESSED: u8 = 0x42;

const ARRAY_MASK: u8 = 1 << 6; // 1 << 6 = 64

type Result<T> = StdResult<T, KbinError>;

pub struct KbinXml {
  options: EncodingOptions,

  offset_1: u64,
  offset_2: u64,
}

impl KbinXml {
  pub fn new() -> Self {
    Self {
      options: EncodingOptions::default(),

      offset_1: 0,
      offset_2: 0,
    }
  }

  pub fn with_options(options: EncodingOptions) -> Self {
    Self {
      options,

      offset_1: 0,
      offset_2: 0,
    }
  }

  #[inline]
  fn data_buf_offset<T>(&self, data_buf: &Cursor<T>) -> u64 {
    // Position is not the index of the previously read byte, it is the current
    // index (offset).
    //
    // This is so much fun to debug.
    //data_buf.position() - 1
    data_buf.position()
  }

  fn data_buf_read(&mut self, data_buf: &mut Cursor<&[u8]>) -> Result<Vec<u8>> {
    let size = data_buf.read_u32::<BigEndian>().context(KbinErrorKind::DataReadSize)?;
    debug!("data_buf_read => index: {}, size: {}", data_buf.position(), size);

    let mut data = vec![0; size as usize];
    data_buf.read_exact(&mut data).context(KbinErrorKind::DataRead)?;
    trace!("data_buf_read => index: {}, size: {}, data: 0x{:02x?}", data_buf.position(), data.len(), data);

    self.data_buf_realign_reads(data_buf, None)?;

    Ok(data)
  }

  fn data_buf_write(&mut self, data_buf: &mut Cursor<Vec<u8>>, data: &[u8]) -> Result<()> {
    data_buf.write_u32::<BigEndian>(data.len() as u32).context(KbinErrorKind::DataWrite("data length integer"))?;
    debug!("data_buf_write => index: {}, size: {}", data_buf.position(), data.len());

    data_buf.write_all(data).context(KbinErrorKind::DataWrite("data block"))?;
    trace!("data_buf_write => index: {}, size: {}, data: 0x{:02x?}", data_buf.position(), data.len(), data);

    self.data_buf_realign_writes(data_buf, None)?;

    Ok(())
  }

  fn data_buf_read_str(&mut self, data_buf: &mut Cursor<&[u8]>, encoding: EncodingType) -> Result<String> {
    let mut data = self.data_buf_read(data_buf)?;

    // Remove trailing null bytes
    let mut index = data.len() - 1;
    let len = data.len();
    while index < len && data[index] == 0x00 {
      index -= 1;
    }
    data.truncate(index + 1);
    trace!("data_buf_read_str => size: {}, data: 0x{:02x?}", data.len(), data);

    encoding.decode_bytes(data)
  }

  fn data_buf_write_str(&mut self, data_buf: &mut Cursor<Vec<u8>>, data: &str, encoding: EncodingType) -> Result<()> {
    trace!("data_buf_write_str => input: {}", data);

    let bytes = encoding.encode_bytes(data)?;
    self.data_buf_write(data_buf, &bytes)?;

    Ok(())
  }

  fn data_buf_get(&mut self, data_buf: &mut Cursor<&[u8]>, size: u32) -> Result<Vec<u8>> {
    let mut data = vec![0; size as usize];
    data_buf.read_exact(&mut data).context(KbinErrorKind::DataRead)?;

    Ok(data)
  }

  fn data_buf_get_aligned(&mut self, data_buf: &mut Cursor<&[u8]>, data_type: KbinType) -> Result<Vec<u8>> {
    if self.offset_1 % 4 == 0 {
      self.offset_1 = self.data_buf_offset(data_buf);
    }
    if self.offset_2 % 4 == 0 {
      self.offset_2 = self.data_buf_offset(data_buf);
    }

    let old_pos = self.data_buf_offset(data_buf);
    let size = data_type.size * data_type.count;
    trace!("data_buf_get_aligned => old_pos: {}, size: {}", old_pos, size);

    let (check_old, data) = match size {
      1 => {
        data_buf.seek(SeekFrom::Start(self.offset_1)).context(KbinErrorKind::Seek)?;

        let data = data_buf.read_u8().context(KbinErrorKind::DataReadOneByte)?;
        self.offset_1 += 1;

        (true, vec![data])
      },
      2 => {
        data_buf.seek(SeekFrom::Start(self.offset_2)).context(KbinErrorKind::Seek)?;

        let mut data = vec![0; 2];
        data_buf.read_exact(&mut data).context(KbinErrorKind::DataReadTwoByte)?;
        self.offset_2 += 2;

        (true, data)
      },
      size => {
        let mut data = vec![0; size as usize];
        data_buf.read_exact(&mut data).context(KbinErrorKind::DataReadAligned)?;
        self.data_buf_realign_reads(data_buf, None)?;

        (false, data)
      },
    };


    if check_old {
      data_buf.seek(SeekFrom::Start(old_pos)).context(KbinErrorKind::Seek)?;

      let trailing = max(self.offset_1, self.offset_2);
      trace!("data_buf_get_aligned => old_pos: {}, trailing: {}", old_pos, trailing);
      if old_pos < trailing {
        data_buf.seek(SeekFrom::Start(trailing)).context(KbinErrorKind::Seek)?;
        self.data_buf_realign_reads(data_buf, None)?;
      }
    }

    Ok(data)
  }

  fn data_buf_write_aligned(&mut self, data_buf: &mut Cursor<Vec<u8>>, data_type: KbinType, data: &[u8]) -> Result<()> {
    if self.offset_1 % 4 == 0 {
      self.offset_1 = self.data_buf_offset(data_buf);
    }
    if self.offset_2 % 4 == 0 {
      self.offset_2 = self.data_buf_offset(data_buf);
    }

    let old_pos = self.data_buf_offset(data_buf);
    let size = (data_type.size as usize) * (data_type.count as usize);
    trace!("data_buf_write_aligned => old_pos: {}, size: {}", old_pos, size);

    if size != data.len() {
      return Err(KbinErrorKind::SizeMismatch(data_type, size, data.len()).into());
    }

    let check_old = match size {
      1 => {
        // Make room for new DWORD
        if self.offset_1 % 4 == 0 {
          data_buf.write_u32::<BigEndian>(0).context(KbinErrorKind::DataWrite("empty DWORD"))?;
        }

        data_buf.seek(SeekFrom::Start(self.offset_1)).context(KbinErrorKind::Seek)?;
        data_buf.write_u8(data[0]).context(KbinErrorKind::DataWrite("1 byte value"))?;
        self.offset_1 += 1;

        true
      },
      2 => {
        // Make room for new DWORD
        if self.offset_2 % 4 == 0 {
          data_buf.write_u32::<BigEndian>(0).context(KbinErrorKind::DataWrite("empty DWORD"))?;
        }

        data_buf.seek(SeekFrom::Start(self.offset_2)).context(KbinErrorKind::Seek)?;
        data_buf.write_u8(data[0]).context(KbinErrorKind::DataWrite("first byte of 2 byte value"))?;
        data_buf.write_u8(data[1]).context(KbinErrorKind::DataWrite("second byte of 2 byte value"))?;
        self.offset_2 += 2;

        true
      },
      _ => {
        data_buf.write_all(data).context(KbinErrorKind::DataWrite("large value"))?;
        self.data_buf_realign_writes(data_buf, None)?;

        false
      },
    };

    if check_old {
      data_buf.seek(SeekFrom::Start(old_pos)).context(KbinErrorKind::Seek)?;

      let trailing = max(self.offset_1, self.offset_2);
      trace!("data_buf_write_aligned => old_pos: {}, trailing: {}", old_pos, trailing);
      if old_pos < trailing {
        data_buf.seek(SeekFrom::Start(trailing)).context(KbinErrorKind::Seek)?;
        self.data_buf_realign_writes(data_buf, None)?;
      }
    }

    Ok(())
  }

  fn data_buf_realign_reads(&self, data_buf: &mut Cursor<&[u8]>, size: Option<u64>) -> Result<()> {
    let size = size.unwrap_or(4);
    trace!("data_buf_realign_reads => position: {}, size: {}", data_buf.position(), size);

    while data_buf.position() % size > 0 {
      data_buf.seek(SeekFrom::Current(1)).context(KbinErrorKind::Seek)?;
    }
    trace!("data_buf_realign_reads => realigned to: {}", data_buf.position());

    Ok(())
  }

  fn data_buf_realign_writes(&self, data_buf: &mut Cursor<Vec<u8>>, size: Option<u64>) -> Result<()> {
    let size = size.unwrap_or(4);
    trace!("data_buf_realign_writes => position: {}, size: {}", data_buf.position(), size);

    while data_buf.position() % size > 0 {
      data_buf.write_u8(0).context(KbinErrorKind::Seek)?;
    }
    trace!("data_buf_realign_writes => realigned to: {}", data_buf.position());

    Ok(())
  }

  fn from_binary_internal(&mut self, stack: &mut Vec<Element>, input: &[u8]) -> Result<(Element, EncodingType)> {
    // Node buffer starts from the beginning.
    // Data buffer starts later after reading `len_data`.
    let mut node_buf = Cursor::new(&input[..]);

    let signature = node_buf.read_u8().context(KbinErrorKind::SignatureRead)?;
    assert_eq!(signature, SIGNATURE);

    // TODO: support uncompressed
    let compress_byte = node_buf.read_u8().context(KbinErrorKind::CompressionRead)?;
    assert_eq!(compress_byte, SIG_COMPRESSED);

    let compressed = Compression::from_byte(compress_byte)?;

    let encoding_byte = node_buf.read_u8().context(KbinErrorKind::EncodingRead)?;
    let encoding_negation = node_buf.read_u8().context(KbinErrorKind::EncodingNegationRead)?;
    let encoding = EncodingType::from_byte(encoding_byte)?;
    assert_eq!(encoding_negation, 0xFF ^ encoding_byte);

    info!("signature: 0x{:x}", signature);
    info!("compression: 0x{:x} ({:?})", compress_byte, compressed);
    info!("encoding: 0x{:x} ({:?})", encoding_byte, encoding);

    let len_node = node_buf.read_u32::<BigEndian>().context(KbinErrorKind::LenNodeRead)?;
    info!("len_node: {} (0x{:x})", len_node, len_node);

    // We have read 8 bytes so far, so offset the start of the data buffer from
    // our current position.
    let data_buf_start = len_node + 8;
    let mut data_buf = Cursor::new(&input[(data_buf_start as usize)..]);

    {
      let pos = data_buf.position();
      self.offset_1 = pos;
      self.offset_2 = pos;
      trace!("offset_1: {}, offset_2: {}", self.offset_1, self.offset_2);
    }

    let len_data = data_buf.read_u32::<BigEndian>().context(KbinErrorKind::LenDataRead)?;
    info!("len_data: {} (0x{:x})", len_data, len_data);

    let node_buf_end = data_buf_start.into();
    while node_buf.position() < node_buf_end {
      let raw_node_type = node_buf.read_u8().context(KbinErrorKind::NodeTypeRead)?;
      let is_array = raw_node_type & 64 == 64;
      let node_type = raw_node_type & !64;

      let xml_type = StandardType::from_u8(node_type);
      debug!("raw_node_type: {}, node_type: {:?} ({}), is_array: {}", raw_node_type, xml_type, node_type, is_array);

      match xml_type {
        StandardType::NodeEnd | StandardType::FileEnd => {
          if stack.len() > 1 {
            let node = stack.pop().expect("Stack must have last node");
            if let Some(to) = stack.last_mut() {
              to.append_child(node);
            }
          }

          if xml_type == StandardType::NodeEnd {
            continue;
          } else if xml_type == StandardType::FileEnd {
            break;
          }
        },
        _ => {},
      };

      let name = unpack_sixbit(&mut node_buf)?;

      if xml_type == StandardType::NodeStart {
        stack.push(Element::bare(name));
      } else {
        if xml_type != StandardType::Attribute {
          stack.push(Element::bare(name.clone()));
        }
        if let Some(to) = stack.last_mut() {
          match xml_type {
            StandardType::Attribute => {
              let val = self.data_buf_read_str(&mut data_buf, encoding)?;
              debug!("attr name: {}, val: {}", name, val);
              to.set_attr(name, val);
            },
            // Removing null bytes is *so much* fun.
            //
            // Handle String nodes separately to use the string reading logic
            // which automatically removes trailing null bytes.
            StandardType::String => {
              to.set_attr("__type", xml_type.name);

              let val = self.data_buf_read_str(&mut data_buf, encoding)?;
              debug!("name: {}, val: {}", name, val);
              to.append_text_node(val);
            },
            _ => {
              to.set_attr("__type", xml_type.name);

              let type_size = xml_type.size;
              let type_count = xml_type.count;
              let (is_array, size) = if type_count == -1 {
                (true, data_buf.read_u32::<BigEndian>().context(KbinErrorKind::BinaryLengthRead)?)
              } else if is_array {
                let node_size = type_size * type_count;
                let arr_count = data_buf.read_u32::<BigEndian>().context(KbinErrorKind::ArrayLengthRead)? / node_size as u32;
                to.set_attr("__count", arr_count);

                let size = (node_size as u32) * arr_count;
                (true, size)
              } else {
                (false, 1)
              };

              debug!("type: {:?}, type_size: {}, type_count: {}, is_array: {}, size: {}",
                xml_type,
                type_size,
                type_count,
                is_array,
                size);

              let data = if is_array {
                let data = self.data_buf_get(&mut data_buf, size)?;
                self.data_buf_realign_reads(&mut data_buf, None)?;

                data
              } else {
                self.data_buf_get_aligned(&mut data_buf, *xml_type)?
              };

              debug!("data: 0x{:02x?}", data);
              if xml_type == StandardType::Binary {
                to.set_attr("__size", data.len());

                let len = data.len() * 2;
                let val = data.into_iter().fold(String::with_capacity(len), |mut val, x| {
                  write!(val, "{:02x}", x).expect("Failed to append hex char");
                  val
                });
                debug!("name: {}, string: {}", name, val);
                to.append_text_node(val);
              } else {
                let inner_value = xml_type.parse_bytes(&data)?;
                debug!("name: {}, string: {}", name, inner_value);
                to.append_text_node(inner_value);
              }
            },
          };
        }
      }
    }

    if stack.len() > 1 {
      warn!("stack: {:#?}", stack);
    }
    stack.truncate(1);

    let element = stack.pop().expect("Stack must have root node");
    Ok((element, encoding))
  }

  fn write_node(&mut self, node_buf: &mut Cursor<Vec<u8>>, data_buf: &mut Cursor<Vec<u8>>, input: &Element) -> Result<()> {
    let encoding = EncodingType::SHIFT_JIS;
    let text = input.text();
    let node_type = match input.attr("__type") {
      Some(name) => StandardType::from_name(name),
      None => {
        if text.len() == 0 {
          StandardType::NodeStart
        } else {
          StandardType::String
        }
      },
    };

    let (array_mask, count) = match input.attr("__count") {
      Some(count) => {
        let count = count.parse::<i8>().context(KbinErrorKind::StringParse("array count"))?;
        debug!("write_node => __count = {}", count);
        (ARRAY_MASK, count)
      },
      None => {
        (0, 1)
      },
    };

    debug!("write_node => name: {}, type: {:?}, type_size: {}, type_count: {}, is_array: {}, size: {}",
      input.name(),
      node_type,
      node_type.size,
      node_type.count,
      array_mask,
      count);

    node_buf.write_u8(node_type.id | array_mask).context(KbinErrorKind::DataWrite(node_type.name))?;
    pack_sixbit(node_buf, input.name())?;

    match node_type {
      StandardType::NodeStart => {},

      StandardType::Binary => {
        let data = text.from_hex().context(KbinErrorKind::HexError)?;
        trace!("data: 0x{:02x?}", data);

        let size = (data.len() as u32) * (node_type.size as u32);
        data_buf.write_u32::<BigEndian>(size).context(KbinErrorKind::DataWrite("binary node size"))?;
        data_buf.write(&data).context(KbinErrorKind::DataWrite("binary"))?;
        self.data_buf_realign_writes(data_buf, None)?;
      },
      StandardType::String => {
        self.data_buf_write_str(data_buf, &text, encoding)?;
      },

      _ => {
        let data = node_type.to_bytes(&text, count as usize)?;
        if array_mask > 0 {
          let total_size = (count as u32) * (node_type.count as u32) * (node_type.size as u32);
          trace!("write_node data_buf array => total_size: {}, data: 0x{:02x?}", total_size, data);

          data_buf.write_u32::<BigEndian>(total_size).context(KbinErrorKind::DataWrite("node size"))?;
          data_buf.write_all(&data).context(KbinErrorKind::DataWrite(node_type.name))?;
          self.data_buf_realign_writes(data_buf, None)?;
        } else {
          self.data_buf_write_aligned(data_buf, *node_type, &data)?;
        }
      },
    }

    for (key, value) in input.attrs() {
      match key {
        "__count" | "__size" | "__type" => continue,
        _ => {},
      };

      trace!("write_node => attr: {}, value: {}", key, value);

      self.data_buf_write_str(data_buf, value, encoding)?;

      let node_type = StandardType::Attribute;
      node_buf.write_u8(node_type.id).context(KbinErrorKind::DataWrite(node_type.name))?;
      pack_sixbit(node_buf, key)?;
    }

    for child in input.children() {
      self.write_node(node_buf, data_buf, child)?;
    }

    // Always has the array bit set
    node_buf.write_u8(StandardType::NodeEnd.id | ARRAY_MASK).context(KbinErrorKind::DataWrite("node end"))?;

    Ok(())
  }

  fn to_binary_internal(&mut self, input: &Element) -> Result<Vec<u8>> {
    let mut header = Cursor::new(Vec::with_capacity(8));
    header.write_u8(SIGNATURE).context(KbinErrorKind::HeaderWrite("signature"))?;
    header.write_u8(SIG_COMPRESSED).context(KbinErrorKind::HeaderWrite("compression"))?;

    let encoding = self.options.encoding.to_byte();
    header.write_u8(encoding).context(KbinErrorKind::HeaderWrite("encoding"))?;
    header.write_u8(0xFF ^ encoding).context(KbinErrorKind::HeaderWrite("encoding negation"))?;

    let mut node_buf = Cursor::new(Vec::new());
    let mut data_buf = Cursor::new(Vec::new());

    self.write_node(&mut node_buf, &mut data_buf, input)?;

    node_buf.write_u8(StandardType::FileEnd.id | ARRAY_MASK).context(KbinErrorKind::DataWrite("file end"))?;
    self.data_buf_realign_writes(&mut node_buf, None)?;

    let mut output = header.into_inner();

    let node_buf = node_buf.into_inner();
    debug!("to_binary_internal => node_buf len: {0} (0x{0:x})", node_buf.len());
    output.write_u32::<BigEndian>(node_buf.len() as u32).context(KbinErrorKind::HeaderWrite("node buffer length"))?;
    output.extend_from_slice(&node_buf);

    let data_buf = data_buf.into_inner();
    debug!("to_binary_internal => data_buf len: {0} (0x{0:x})", data_buf.len());
    output.write_u32::<BigEndian>(data_buf.len() as u32).context(KbinErrorKind::HeaderWrite("data buffer length"))?;
    output.extend_from_slice(&data_buf);

    Ok(output)
  }

  pub fn from_binary(input: &[u8]) -> Result<(Element, EncodingType)> {
    let mut kbinxml = KbinXml::new();
    let mut stack: Vec<Element> = Vec::new();

    kbinxml.from_binary_internal(&mut stack, input).map_err(|e| {
      if let Some(first) = stack.first() {
        println!("{:?}", first);
      }
      e
    })
  }

  pub fn to_binary(input: &Element) -> Result<Vec<u8>> {
    let mut kbinxml = KbinXml::new();
    kbinxml.to_binary_internal(input)
  }

  pub fn to_binary_with_options(options: EncodingOptions, input: &Element) -> Result<Vec<u8>> {
    let mut kbinxml = KbinXml::with_options(options);
    kbinxml.to_binary_internal(input)
  }
}
