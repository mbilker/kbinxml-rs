use std::fs::File;
use std::io::{self, Error as IoError, Read, Write};

use byteorder::{BigEndian, ByteOrder};
use bytes::Bytes;
use clap::{App, Arg};
use failure::Fallible;
use kbinxml::{NodeCollection, Options, Printer};
use minidom::Element;
use quick_xml::Reader;

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

fn run() -> Fallible<()> {
  let matches = App::new("kbinxml")
    .about(env!("CARGO_PKG_DESCRIPTION"))
    .version(env!("CARGO_PKG_VERSION"))
    .author("Matt Bilker <me@mbilker.us>")
    .arg(Arg::with_name("printer")
      .help("Turn on the NodeCollection and NodeDefinition debug printer")
      .short("p"))
    .arg(Arg::with_name("input")
      .help("The file to convert")
      .index(1))
    .get_matches();

  let printer_enabled = matches.is_present("printer");

  if let Some(file_name) = matches.value_of("input") {
    eprintln!("file_name: {}", file_name);

    let mut contents = Vec::new();

    // Read '-' as standard input.
    if file_name == "-" {
      io::stdin().read_to_end(&mut contents)?;
    } else {
      let mut file = File::open(file_name)?;
      file.read_to_end(&mut contents)?;
    }

    if kbinxml::is_binary_xml(&contents) {
      if printer_enabled {
        Printer::run(&contents).unwrap();
      }

      let (collection, _encoding) = kbinxml::from_slice(&contents)?;
      let text_original = kbinxml::to_text_xml(&collection)?;
      display_buf(&text_original)?;

      let (element, encoding_original) = kbinxml::element_from_binary(&contents)?;

      let options = Options::with_encoding(encoding_original);
      let buf = kbinxml::to_binary_with_options(options, &element)?;
      compare_slice(&buf, &contents);
    } else {
      let (collection, encoding) = kbinxml::from_text_xml(&contents)?;

      let mut reader = Reader::from_reader(contents.as_slice());
      let element = Element::from_reader(&mut reader).expect("Unable to construct DOM for input text XML");

      let options = Options::with_encoding(encoding);
      let buf = kbinxml::to_binary_with_options(options, &element)?;
      eprintln!("data: {:02x?}", buf);

      if printer_enabled {
        Printer::run(&buf)?;
      }

      let (encoded_collection, _encoding) = kbinxml::from_binary(Bytes::from(buf.clone()))?;
      compare_collections(&collection, &encoded_collection);

      io::stdout().write_all(&buf)?;
    }
  } else {
    eprintln!("No input file specified!");
  }

  Ok(())
}

fn main() {
  pretty_env_logger::init();

  if let Err(e) = run() {
    eprintln!("Error: {}", e);

    for cause in e.iter_causes() {
      eprintln!("Cause: {}", cause);
    }

    eprintln!("{}", e.backtrace());
  }
}
