use std::fs;
use std::io::{self, Error as IoError, Read, Write};

use anyhow::Context;
use byteorder::{BigEndian, ByteOrder};
use clap::{App, Arg};
use encoding_rs::Encoding;
use kbinxml::{EncodingType, Options, Printer};

fn display_buf(buf: &[u8]) -> Result<(), IoError> {
  io::stdout().write_all(&buf)?;
  println!();

  Ok(())
}

fn compare_slice(left: &[u8], right: &[u8]) {
  let node_buf_length = BigEndian::read_u32(&left[4..8]);
  let data_buf_start = 8 + node_buf_length as usize;

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

fn main() -> Result<(), anyhow::Error> {
  pretty_env_logger::init();

  let matches = App::new("kbinxml")
    .about(env!("CARGO_PKG_DESCRIPTION"))
    .version(env!("CARGO_PKG_VERSION"))
    .author("Matt Bilker <me@mbilker.us>")
    .arg(Arg::with_name("printer")
      .help("Turn on the NodeCollection and NodeDefinition debug printer")
      .short("p")
      .long("printer"))
    .arg(Arg::with_name("encoding")
      .help("Set the encoding used when encoding kbin data")
      .short("e")
      .long("encoding")
      .takes_value(true))
    .arg(Arg::with_name("input")
      .help("The file to convert")
      .index(1)
      .required(true))
    .get_matches();

  let printer_enabled = matches.is_present("printer");
  let file_name = matches.value_of("input").unwrap();
  let output_encoding = if let Some(label) = matches.value_of("encoding") {
    let encoding = Encoding::for_label(label.as_bytes())
      .with_context(|| "No encoding found for label")?;

    Some(EncodingType::from_encoding(encoding)?)
  } else {
    None
  };

  eprintln!("file_name: {}", file_name);

  // Read '-' as standard input.
  let contents = if file_name == "-" {
    let mut contents = Vec::new();
    io::stdin().read_to_end(&mut contents)?;

    contents
  } else {
    fs::read(file_name)?
  };

  if kbinxml::is_binary_xml(&contents) {
    if printer_enabled {
      Printer::run(&contents).unwrap();
    }

    let (collection, _encoding) = kbinxml::from_slice(&contents)?;
    let text_original = kbinxml::to_text_xml(&collection)?;
    display_buf(&text_original)?;

    let (collection, encoding_original) = kbinxml::from_slice(&contents)?;
    let options = Options::with_encoding(output_encoding.unwrap_or(encoding_original));
    let buf = kbinxml::to_binary_with_options(options, &collection)?;
    compare_slice(&buf, &contents);
  } else {
    let (collection, encoding) = kbinxml::from_text_xml(&contents)?;
    let options = Options::with_encoding(output_encoding.unwrap_or(encoding));
    let buf = kbinxml::to_binary_with_options(options, &collection)?;

    if printer_enabled {
      Printer::run(&buf)?;
    }

    io::stdout().write_all(&buf)?;
  }

  Ok(())
}
