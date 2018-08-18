#![feature(int_to_from_bytes)]

extern crate failure;
extern crate kbinxml;
extern crate minidom;
extern crate pretty_env_logger;
extern crate quick_xml;

#[macro_use] extern crate serde_derive;

use std::env;
use std::fs::File;
use std::io::{Cursor, Error as IoError, ErrorKind as IoErrorKind, Read, Write, stdout};
use std::net::Ipv4Addr;
use std::str;

use failure::Fail;
use kbinxml::{Ip4Addr, KbinXml, Node, Options, Printer, Value, from_bytes, to_bytes};
use minidom::Element;
use quick_xml::Writer;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "test2")]
pub struct Testing2 {
  hi: u16,
  ho: i16,
  vu: Vec<u8>,
  opt: Option<u8>,
  opt2: Option<u8>,
  ip: Ip4Addr,

  #[serde(flatten)]
  extra: Value,
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

fn to_text(element: &Element) -> Result<Vec<u8>, IoError> {
  let inner = Cursor::new(Vec::new());
  let mut writer = Writer::new_with_indent(inner, b' ', 2);
  element.to_writer(&mut writer).map_err(|e| IoError::new(IoErrorKind::Other, format!("{:?}", e)))?;

  let buf = writer.into_inner().into_inner();
  Ok(buf)
}

fn display_buf(buf: &[u8]) -> Result<(), IoError> {
  let stdout = stdout();
  stdout.lock().write_all(&buf)?;
  println!();

  Ok(())
}

fn compare_slice(left: &[u8], right: &[u8]) {
  let mut buf = [0; 4];
  buf.clone_from_slice(&left[4..8]);
  let node_buf_length = u32::from_be(u32::from_bytes(buf));
  //println!("node_buf_length: {}", node_buf_length);

  let data_buf_start = 8 + node_buf_length as usize;
  let data_buf_len_end = data_buf_start + 4;
  //println!("data_buf start: {} + 8 = {}", node_buf_length, data_buf_start);

  buf.clone_from_slice(&left[data_buf_start..data_buf_len_end]);
  //let data_buf_length = u32::from_be(u32::from_bytes(buf));
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

fn main() -> std::io::Result<()> {
  pretty_env_logger::init();

  if let Some(file_name) = env::args().skip(1).next() {
    eprintln!("file_name: {}", file_name);

    let mut file = File::open(file_name)?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents)?;

    if KbinXml::is_binary_xml(&contents) {
      Printer::run(&contents).unwrap();

      let (element, encoding_original) = KbinXml::from_binary(&contents).map_err(display_err)?;
      let text_original = to_text(&element)?;
      display_buf(&text_original)?;

      let options = Options::with_encoding(encoding_original);
      let buf = KbinXml::to_binary_with_options(options, &element).map_err(display_err)?;
      compare_slice(&buf, &contents);

      let value = from_bytes::<Node>(&contents);
      match &value {
        Ok(obj2) => eprintln!("obj2: {:#?}", obj2),
        Err(e) => eprintln!("Unable to parse generated kbin back to `Value`: {:#?}", e),
      };
    } else {
      let contents = str::from_utf8(&contents).expect("Unable to interpret file contents as UTF-8");
      let element: Element = contents.parse().expect("Unable to construct DOM for input text XML");

      let options = Options::default();
      let buf = KbinXml::to_binary_with_options(options, &element).map_err(display_err)?;
      eprintln!("data: {:02x?}", buf);
      Printer::run(&buf).unwrap();

      let mut stdout = stdout();
      stdout.lock().write_all(&buf)?;
    }
  } else {
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
        ip: Ip4Addr::new(Ipv4Addr::new(127, 0, 0, 1)),
        extra: Value::Map(Default::default()),
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
  }
  Ok(())
}
