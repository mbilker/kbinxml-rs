extern crate failure;
extern crate kbinxml;
extern crate pretty_env_logger;
extern crate quick_xml;

use std::env;
use std::fs::File;
use std::io::{Cursor, Error as IoError, ErrorKind as IoErrorKind, Read, Write, stdout};

use failure::Fail;
use kbinxml::KbinXml;
use quick_xml::Writer;

fn display_err(err: impl Fail) -> IoError {
  let mut fail: &Fail = &err;
  while let Some(cause) = fail.cause() {
    eprintln!("Cause: {}", cause);
    fail = cause;
  }

  if let Some(backtrace) = err.cause().and_then(|cause| cause.backtrace()) {
    eprintln!("{}", backtrace);
  }

  IoError::new(IoErrorKind::Other, "Error parsing kbin")
}

fn main() -> std::io::Result<()> {
  pretty_env_logger::init();

  if let Some(file_name) = env::args().skip(1).next() {
    eprintln!("file_name: {}", file_name);

    let mut file = File::open(file_name)?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents)?;

    let element = KbinXml::from_binary(&contents).map_err(display_err)?;
    //println!("element: {:#?}", element);

    let inner = Cursor::new(Vec::new());
    let mut writer = Writer::new_with_indent(inner, b' ', 2);
    element.to_writer(&mut writer).map_err(|e| IoError::new(IoErrorKind::Other, format!("{:?}", e)))?;

    let buf = writer.into_inner().into_inner();
    let stdout = stdout();
    stdout.lock().write_all(&buf)?;
    println!();

    let buf = KbinXml::to_binary(&element).map_err(display_err)?;
    assert_eq!(buf, contents);
  }
  Ok(())
}
