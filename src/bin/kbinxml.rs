#![feature(int_to_from_bytes)]

extern crate failure;
extern crate kbinxml;
extern crate minidom;
extern crate pretty_env_logger;
extern crate quick_xml;

use std::env;
use std::fs::File;
use std::io::{Cursor, Error as IoError, ErrorKind as IoErrorKind, Read, Write, stdout};

use failure::Fail;
use kbinxml::{EncodingOptions, KbinXml};
use minidom::Element;
use quick_xml::Writer;

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

fn display_element(element: &Element) -> Result<(), IoError> {
  let inner = Cursor::new(Vec::new());
  let mut writer = Writer::new_with_indent(inner, b' ', 2);
  element.to_writer(&mut writer).map_err(|e| IoError::new(IoErrorKind::Other, format!("{:?}", e)))?;

  let buf = writer.into_inner().into_inner();
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
    eprintln!(r#"  left: `{:?}`
 right: `{:?}`"#, &left[*i..], &right[*i..]);
  }
}

fn main() -> std::io::Result<()> {
  pretty_env_logger::init();

  if let Some(file_name) = env::args().skip(1).next() {
    eprintln!("file_name: {}", file_name);

    let mut file = File::open(file_name)?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents)?;

    let (element, encoding) = KbinXml::from_binary(&contents).map_err(display_err)?;
    //println!("element: {:#?}", element);
    display_element(&element)?;

    let options = EncodingOptions::with_encoding(encoding);
    let buf = KbinXml::to_binary_with_options(options, &element).map_err(display_err)?;
    compare_slice(&buf, &contents);

    let (element, new_encoding) = KbinXml::from_binary(&buf).map_err(display_err)?;
    display_element(&element)?;
    assert_eq!(encoding, new_encoding);
  }
  Ok(())
}
