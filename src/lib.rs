#![feature(int_to_from_bytes)]

extern crate byteorder;
extern crate encoding;
extern crate minidom;
extern crate num;

#[macro_use] extern crate lazy_static;
#[macro_use] extern crate log;

use std::cmp::max;
use std::fmt::Write;
use std::io::{Cursor, Read, Seek, SeekFrom};

use byteorder::{BigEndian, ReadBytesExt};
use minidom::Element;

mod compression;
mod encoding_type;
mod node_types;
mod sixbit;

use compression::Compression;
use encoding_type::EncodingType;
use node_types::KbinType;
use sixbit::unpack_sixbit;

const SIGNATURE: u8 = 0xA0;

const SIG_COMPRESSED: u8 = 0x42;

pub struct KbinXml {
  offset_1: u64,
  offset_2: u64,
}

impl KbinXml {
  pub fn new() -> Self {
    Self {
      offset_1: 0,
      offset_2: 0,
    }
  }

  #[inline]
  fn data_buf_offset(&self, data_buf: &Cursor<&[u8]>) -> u64 {
    // Position is not the index of the previously read byte, it is the current
    // index (offset).
    //
    // This is so much fun to debug.
    //data_buf.position() - 1
    data_buf.position()
  }

  fn data_buf_read(&mut self, data_buf: &mut Cursor<&[u8]>) -> Vec<u8> {
    let size = data_buf.read_i32::<BigEndian>().expect("Unable to read data size");
    let mut data = vec![0; size as usize];
    data_buf.read_exact(&mut data).expect("Unable to read data");
    println!("data_buf_read => size: {}, data: 0x{:02x?}", data.len(), data);

    self.data_buf_realign(data_buf, None);

    data
  }

  fn data_buf_read_str(&mut self, data_buf: &mut Cursor<&[u8]>, encoding: EncodingType) -> String {
    let mut data = self.data_buf_read(data_buf);

    // Remove trailing null bytes
    let mut index = data.len() - 1;
    while data[index] == 0x00 {
      index -= 1;
    }
    data.truncate(index + 1);
    println!("data_buf_read_str => size: {}, data: 0x{:02x?}", data.len(), data);

    //String::from_utf8(data).expect("Unable to interpret string node as UTF-8")
    encoding.decode_bytes(data)
  }

  fn data_buf_get(&mut self, data_buf: &mut Cursor<&[u8]>, size: u32) -> Vec<u8> {
    let mut data = vec![0; size as usize];
    data_buf.read_exact(&mut data).expect("Unable to read data");

    data
  }

  fn data_buf_get_aligned(&mut self, data_buf: &mut Cursor<&[u8]>, data_type: KbinType) -> Vec<u8> {
    if self.offset_1 % 4 == 0 {
      self.offset_1 = self.data_buf_offset(data_buf);
    }
    if self.offset_2 % 4 == 0 {
      self.offset_2 = self.data_buf_offset(data_buf);
    }

    let old_pos = self.data_buf_offset(data_buf);
    let size = data_type.size() * data_type.count();
    println!("data_buf_get_aligned => old_pos: {}, size: {}", old_pos, size);
    let (check_old, data) = match size {
      1 => {
        data_buf.seek(SeekFrom::Start(self.offset_1)).expect("Unable to seek data buffer");

        let data = data_buf.read_u8().expect("Unable to read 1 byte data");
        self.offset_1 += 1;

        (true, vec![data])
      },
      2 => {
        data_buf.seek(SeekFrom::Start(self.offset_2)).expect("Unable to seek data buffer");

        let mut data = vec![0; 2];
        data_buf.read_exact(&mut data).expect("Unable to read 2 byte data");
        self.offset_2 += 2;

        (true, data)
      },
      size => {
        let mut data = vec![0; size as usize];
        data_buf.read_exact(&mut data).expect("Unable to read aligned data from data buffer");
        self.data_buf_realign(data_buf, None);

        (false, data)
      },
    };


    if check_old {
      data_buf.seek(SeekFrom::Start(old_pos)).expect("Unable to seek data buffer");

      let trailing = max(self.offset_1, self.offset_2);
      println!("old_pos: {}, trailing: {}", old_pos, trailing);
      if old_pos < trailing {
        data_buf.seek(SeekFrom::Start(trailing)).expect("Unable to seek data buffer");
        self.data_buf_realign(data_buf, None);
      }
    }

    data
  }

  fn data_buf_realign(&mut self, data_buf: &mut Cursor<&[u8]>, size: Option<u64>) {
    let size = size.unwrap_or(4);
    println!("data_buf_realign => position: {}, size: {}", data_buf.position(), size);

    while data_buf.position() % size > 0 {
      data_buf.seek(SeekFrom::Current(1)).expect("Unable to seek data buffer");
    }
    println!("data_buf_realign => realigned to: {}", data_buf.position());
  }

  fn from_binary_internal(&mut self, input: &[u8]) -> Element {
    // Node buffer starts from the beginning.
    // Data buffer starts later after reading `len_data`.
    let mut node_buf = Cursor::new(&input[..]);

    let signature = node_buf.read_u8().expect("Unable to read signature byte");
    assert_eq!(signature, SIGNATURE);

    // TODO: support uncompressed
    let compress_byte = node_buf.read_u8().expect("Unable to read compression byte");
    assert_eq!(compress_byte, SIG_COMPRESSED);

    let compressed = Compression::from_byte(compress_byte).expect("Unknown compression value");

    let encoding_byte = node_buf.read_u8().expect("Unable to read encoding byte");
    let encoding = EncodingType::from_byte(encoding_byte).expect("Unknown encoding");

    let encoding_negation = node_buf.read_u8().expect("Unable to read encoding negation byte");
    assert_eq!(encoding_negation, 0xFF ^ encoding_byte);

    println!("signature: 0x{:x}", signature);
    println!("compression: 0x{:x} ({:?})", compress_byte, compressed);
    println!("encoding: 0x{:x} ({:?})", encoding_byte, encoding);

    let len_node = node_buf.read_u32::<BigEndian>().expect("Unable to read len_node");
    println!("len_node: {} (0x{:x})", len_node, len_node);

    // We have read 8 bytes so far, so offset the start of the data buffer from
    // our current position.
    let data_buf_start = len_node + 8;
    let mut data_buf = Cursor::new(&input[(data_buf_start as usize)..]);

    {
      let pos = data_buf.position();
      self.offset_1 = pos;
      self.offset_2 = pos;
      println!("offset_1: {}, offset_2: {}", self.offset_1, self.offset_2);
    }

    let len_data = data_buf.read_u32::<BigEndian>().expect("Unable to read len_data");
    println!("len_data: {} (0x{:x})", len_data, len_data);

    let root = Element::bare("root");
    let mut stack = vec![root];
    {
      let node_buf_end = data_buf_start.into();
      while node_buf.position() < node_buf_end {
        let raw_node_type = node_buf.read_u8().expect("Unable to read node type");
        let is_array = raw_node_type & 64 == 64;
        let node_type = raw_node_type & !64;

        let xml_type = KbinType::from_u8(node_type);
        println!("raw_node_type: {}, node_type: {:?} ({}), is_array: {}", raw_node_type, xml_type, node_type, is_array);

        match xml_type {
          KbinType::NodeEnd => {
            if stack.len() > 1 {
              let node = stack.pop().expect("Stack must have last node");
              if let Some(to) = stack.last_mut() {
                to.append_child(node);
              }
            }
            continue;
          },
          KbinType::FileEnd => {
            if stack.len() > 1 {
              let node = stack.pop().expect("Stack must have last node");
              if let Some(to) = stack.last_mut() {
                to.append_child(node);
              }
            }
            break;
          },
          _ => {},
        };

        let name = unpack_sixbit(&mut node_buf);
        if xml_type == KbinType::NodeStart {
          stack.push(Element::bare(name));
        } else {
          if xml_type != KbinType::Attribute {
            stack.push(Element::bare(name.clone()));
          }
          if let Some(to) = stack.last_mut() {
            match xml_type {
              KbinType::Attribute => {
                let val = self.data_buf_read_str(&mut data_buf, encoding);
                println!("attr name: {}, val: {}", name, val);
                to.set_attr(name, val);
              },
              _ => {
                to.set_attr("__type", xml_type.name());

                let type_size = xml_type.size();
                let type_count = xml_type.count();
                let (is_array, size) = if xml_type.count() == -1 {
                  println!("xml_type.count() == -1");
                  (true, data_buf.read_u32::<BigEndian>().expect("Unable to read binary/string byte length"))
                } else if is_array {
                  let node_size = type_size * type_count;
                  let arr_count = data_buf.read_u32::<BigEndian>().expect("Unable to read array node length") / node_size as u32;
                  to.set_attr("__count", arr_count);

                  let size = (node_size as u32) * arr_count;
                  (true, size)
                } else {
                  (false, 1)
                };

                println!("type: {:?}, type_size: {}, type_count: {}, is_array: {}, size: {}",
                  xml_type,
                  type_size,
                  type_count,
                  is_array,
                  size);

                let data = if is_array {
                  let data = self.data_buf_get(&mut data_buf, size);
                  self.data_buf_realign(&mut data_buf, None);

                  data
                } else {
                  self.data_buf_get_aligned(&mut data_buf, xml_type)
                };
                println!("data: 0x{:02x?}", data);
                if xml_type == KbinType::String {
                  let val = encoding.decode_bytes(data);
                  println!("name: {}, string: {}", name, val);
                  to.append_text_node(val);
                } else if xml_type == KbinType::Binary {
                  let len = data.len() * 2;
                  let val = data.into_iter().fold(String::with_capacity(len), |mut val, x| {
                    write!(val, "{:02x}", x).expect("Failed to append hex char");
                    val
                  });
                  println!("name: {}, string: {}", name, val);
                  to.append_text_node(val);
                } else {
                  let inner_value = xml_type.parse_bytes(&data);
                  println!("name: {}, string: {}", name, inner_value);
                  to.append_text_node(inner_value);
                }
              },
            };
          }
        }
      }
    }
    if stack.len() > 1 {
      println!("stack: {:#?}", stack);
    }
    stack.truncate(1);
    stack.pop().expect("Stack must have root node")
  }

  pub fn from_binary(input: &[u8]) -> Element {
    let mut kbinxml = KbinXml::new();
    kbinxml.from_binary_internal(input)
  }
}
