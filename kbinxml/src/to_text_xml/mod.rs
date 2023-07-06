use std::io::{Cursor, Write};

use quick_xml::events::{BytesDecl, Event};
use quick_xml::Writer;

use crate::encoding_type::EncodingType;
use crate::error::KbinError;

mod node;
mod node_collection;

pub trait ToTextXml {
    fn encoding(&self) -> EncodingType;
    fn write<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), KbinError>;
}

pub struct TextXmlWriter {
    xml_writer: Writer<Cursor<Vec<u8>>>,
}

impl TextXmlWriter {
    pub fn new() -> Self {
        let inner = Cursor::new(Vec::new());
        let xml_writer = Writer::new_with_indent(inner, b' ', 2);

        Self { xml_writer }
    }

    pub fn into_text_xml<T>(mut self, value: &T) -> Result<Vec<u8>, KbinError>
    where
        T: ToTextXml,
    {
        if let Some(encoding) = value.encoding().name() {
            let header = BytesDecl::new("1.0", Some(encoding), None);

            self.xml_writer.write_event(Event::Decl(header))?;
        }

        value.write(&mut self.xml_writer)?;

        Ok(self.xml_writer.into_inner().into_inner())
    }
}
