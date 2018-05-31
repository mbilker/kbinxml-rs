#![feature(int_to_from_bytes)]

extern crate byteorder;
extern crate minidom;
extern crate num;

#[macro_use] extern crate lazy_static;
#[macro_use] extern crate log;

use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;

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
}

impl KbinXml {
  pub fn new() -> Self {
    Self {
    }
  }

  pub fn from_binary(input: &[u8]) -> Element {
    // Node buffer starts from the beginning.
    // Data buffer starts later after reading `len_data`.
    let mut node_buf = Cursor::new(&input[..]);

    let signature = node_buf.read_u8().expect("Unable to read signature byte");
    assert_eq!(signature, SIGNATURE);

    // TODO: support uncompressed
    let compress_byte = node_buf.read_u8().expect("Unable to read compression byte");
    assert_eq!(compress_byte, SIG_COMPRESSED);

    let compressed = Compression::from_byte(compress_byte);
    assert!(compressed.is_some());

    let encoding_byte = node_buf.read_u8().expect("Unable to read encoding byte");
    let encoding = EncodingType::from_byte(encoding_byte);
    assert!(encoding.is_some());

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
                to.set_attr(name, "");
              },
              _ => {
                to.set_attr("__type", xml_type.name());
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
}
