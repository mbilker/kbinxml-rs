extern crate kbinxml;
extern crate pretty_env_logger;
extern crate quick_xml;

use std::env;
use std::fs::File;
use std::io::{Cursor, Error as IoError, ErrorKind as IoErrorKind, Read, Write, stdout};

use kbinxml::KbinXml;
use quick_xml::Writer;

fn main() -> std::io::Result<()> {
  pretty_env_logger::init();

  if let Some(file_name) = env::args().skip(1).next() {
    println!("file_name: {}", file_name);

    let mut file = File::open(file_name)?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents)?;

    let element = KbinXml::from_binary(&contents);
    //println!("element: {:#?}", element);

    let inner = Cursor::new(Vec::new());
    let mut writer = Writer::new_with_indent(inner, b' ', 2);
    element.to_writer(&mut writer).map_err(|e| IoError::new(IoErrorKind::Other, format!("{:?}", e)))?;

    let buf = writer.into_inner().into_inner();
    let stdout = stdout();
    stdout.lock().write_all(&buf)?;
    println!();
  }
  Ok(())
}
