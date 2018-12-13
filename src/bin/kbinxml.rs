extern crate byteorder;
extern crate failure;
extern crate kbinxml;
extern crate minidom;
extern crate pretty_env_logger;
extern crate quick_xml;

#[macro_use] extern crate cfg_if;

use std::env;
use std::fs::File;
use std::io::{self, Error as IoError, ErrorKind as IoErrorKind, Read, Write};

use byteorder::{BigEndian, ByteOrder};
use failure::Fail;
use kbinxml::{NodeCollection, Options, Printer};
use minidom::Element;
use quick_xml::Reader;

cfg_if! {
  if #[cfg(feature = "serde")] {
    #[macro_use] extern crate serde_derive;

    use std::net::Ipv4Addr;

    use kbinxml::{ExtraNodes, Node, from_bytes, to_bytes};

    #[derive(Debug, Deserialize, Serialize)]
    #[serde(rename = "test2")]
    pub struct Testing2 {
      hi: u16,
      ho: i16,
      vu: Vec<u8>,
      opt: Option<u8>,
      opt2: Option<u8>,
      ip: Ipv4Addr,

      #[serde(flatten)]
      extra: ExtraNodes,
    }

    #[derive(Debug, Deserialize, Serialize)]
    #[serde(rename = "test")]
    pub struct Testing {
      #[serde(rename = "attr_the_attr")] the_attr: String,
      hi: u8,
      ok: [u8; 3],
      hhh: (u8, u8),
      hhg: (u32, u32),
      foo: String,
      testing2: Testing2,
    }
  }
}

fn display_err(err: impl Fail) -> IoError {
  let mut fail: &Fail = &err;
  eprintln!("e: {}", err);
  while let Some(cause) = fail.cause() {
    eprintln!("Cause: {}", cause);
    fail = cause;
  }

  if let Some(backtrace) = err.backtrace() {
    eprintln!("{}", backtrace);
  }

  IoError::new(IoErrorKind::Other, "Error parsing kbin")
}

fn display_buf(buf: &[u8]) -> Result<(), IoError> {
  io::stdout().write_all(&buf)?;
  println!();

  Ok(())
}

fn compare_collections(left: &NodeCollection, right: &NodeCollection) -> bool {
  if left.base() != right.base() {
    eprintln!("left.base() != right.base()");
    eprintln!("left.base(): {:#?}", left.base());
    eprintln!("right.base(): {:#?}", right.base());

    return false;
  }

  for (left, right) in left.attributes().iter().zip(right.attributes().iter()) {
    if left != right {
      eprintln!("left attribute != right attribute");
      eprintln!("left: {:#?}", left);
      eprintln!("right: {:#?}", right);

      return false;
    }
  }

  for (left, right) in left.children().iter().zip(right.children().iter()) {
    if !compare_collections(left, right) {
      return false;
    }
  }

  true
}

fn compare_slice(left: &[u8], right: &[u8]) {
  let node_buf_length = BigEndian::read_u32(&left[4..8]);
  //println!("node_buf_length: {}", node_buf_length);

  let data_buf_start = 8 + node_buf_length as usize;
  //let data_buf_len_end = data_buf_start + 4;
  //println!("data_buf start: {} + 8 = {}", node_buf_length, data_buf_start);

  //let data_buf_length = BigEndian::read_u32(&left[data_buf_start..data_buf_len_end]);
  //println!("data_buf_length: {}", data_buf_length);

  let mut i = 0;
  let mut mismatches = Vec::new();
  while i < left.len() && i < right.len() {
    if left[i] != right[i] {
      mismatches.push((i, left[i], right[i]));
    }
    i += 1;
  }

  if let Some(ref first) = mismatches.first() {
    eprintln!("Left does not equal right at the following indexes:");
    for (i, left, right) in &mismatches {
      let (section, offset) = if *i < data_buf_start {
        ("node buffer", (*i as isize) - 8)
      } else {
        ("data buffer", (*i as isize) - 4 - (data_buf_start as isize))
      };
      eprintln!("index {0} ({3}, offset: {4}), left: {1:3} (0x{1:x}),\tright: {2:3} (0x{2:x})", i, left, right, section, offset);
    }

    let (i, _, _) = first;
    eprintln!(r#"  left: `0x{:02x?}`
 right: `0x{:02x?}`"#, &left[*i..], &right[*i..]);
  }
}

#[cfg(feature = "serde")]
fn test_serde() -> std::io::Result<()> {
  let obj = Testing {
    the_attr: "the_value".to_string(),
    hi: 12,
    ok: [12, 24, 48],
    hhh: (55, 66),
    hhg: (55, 66),
    foo: "foobarbaz".to_string(),
    testing2: Testing2 {
      hi: 32423,
      ho: 32000,
      vu: vec![33, 255, 254],
      opt: None,
      opt2: Some(111),
      ip: Ipv4Addr::new(127, 0, 0, 1),
      extra: ExtraNodes::new(),
    },
  };
  let bytes = to_bytes(&obj).unwrap();
  eprintln!("bytes: {:02x?}", bytes);

  let mut file = File::create("testing.kbin")?;
  file.write_all(&bytes)?;

  let obj2 = from_bytes::<Testing>(&bytes);
  match &obj2 {
    Ok(obj2) => eprintln!("obj2: {:#?}", obj2),
    Err(e) => eprintln!("Unable to parse generated kbin back to struct: {:#?}", e),
  };

  let value = from_bytes::<Node>(&bytes);
  match &value {
    Ok(obj2) => eprintln!("obj2: {:#?}", obj2),
    Err(e) => eprintln!("Unable to parse generated kbin back to `Value`: {:#?}", e),
  };

  if obj2.is_ok() && value.is_ok() {
    Printer::run(&bytes).unwrap();
  }

  Ok(())
}

#[cfg(not(feature = "serde"))]
fn test_serde() -> std::io::Result<()> {
  Ok(())
}

#[cfg(feature = "serde")]
fn test_serde_node(contents: &[u8]) -> std::io::Result<()> {
  let node = from_bytes::<Node>(&contents);
  match &node {
    Ok(obj2) => {
      eprintln!("obj2: {:#?}", obj2);
    },
    Err(e) => eprintln!("Unable to parse generated kbin back to `Node`: {:#?}", e),
  };

  Ok(())
}

#[cfg(not(feature = "serde"))]
fn test_serde_node(_contents: &[u8]) -> std::io::Result<()> {
  Ok(())
}

fn main() -> std::io::Result<()> {
  pretty_env_logger::init();

  if let Some(file_name) = env::args().skip(1).next() {
    eprintln!("file_name: {}", file_name);

    let mut file = File::open(file_name)?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents)?;

    if kbinxml::is_binary_xml(&contents) {
      Printer::run(&contents).unwrap();

      let (collection, _encoding) = kbinxml::from_slice(&contents).map_err(display_err)?;
      let text_original = kbinxml::to_text_xml(&collection).map_err(display_err)?;
      display_buf(&text_original)?;

      let (element, encoding_original) = kbinxml::element_from_binary(&contents).map_err(display_err)?;

      let options = Options::with_encoding(encoding_original);
      let buf = kbinxml::to_binary_with_options(options, &element).map_err(display_err)?;
      compare_slice(&buf, &contents);

      test_serde_node(&contents)?;
    } else {
      let mut reader = Reader::from_reader(contents.as_slice());
      let element = Element::from_reader(&mut reader).expect("Unable to construct DOM for input text XML");

      let options = Options::default();
      let buf = kbinxml::to_binary_with_options(options, &element).map_err(display_err)?;
      eprintln!("data: {:02x?}", buf);

      let encoded_collection = Printer::run(&buf).unwrap();
      let (collection, _encoding) = kbinxml::from_text_xml(&contents).map_err(display_err)?;

      if let Some(encoded_collection) = encoded_collection {
        compare_collections(&encoded_collection, &collection);
      }

      io::stdout().write_all(&buf)?;
    }
  } else {
    test_serde()?;
  }
  Ok(())
}
