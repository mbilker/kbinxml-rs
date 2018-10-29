use std::io::{Cursor, Write};

use quick_xml::Writer;

use error::KbinError;

mod node;
mod node_collection;

pub trait ToTextXml {
  fn write<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), KbinError>;
}

pub struct TextXmlWriter {
  xml_writer: Writer<Cursor<Vec<u8>>>,
}

impl TextXmlWriter {
  pub fn new() -> Self {
    let inner = Cursor::new(Vec::new());
    let xml_writer = Writer::new_with_indent(inner, b' ', 2);

    Self {
      xml_writer,
    }
  }

  pub fn to_text_xml<T>(mut self, value: &T) -> Result<Vec<u8>, KbinError>
    where T: ToTextXml
  {
    value.write(&mut self.xml_writer)?;

    Ok(self.xml_writer.into_inner().into_inner())
  }
}
