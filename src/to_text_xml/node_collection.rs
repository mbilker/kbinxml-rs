use std::borrow::Cow;
use std::io::Write;

use quick_xml::Writer;
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::events::attributes::Attribute;

use crate::encoding_type::EncodingType;
use crate::error::KbinError;
use crate::node::NodeCollection;
use crate::node_types::StandardType;
use crate::to_text_xml::ToTextXml;

impl ToTextXml for NodeCollection {
  /// At the moment, decoding the value of a `NodeDefinition` will decode
  /// strings into UTF-8.
  fn encoding(&self) -> EncodingType {
    EncodingType::UTF_8
  }

  fn write<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), KbinError> {
    let base = self.base();
    let key = base.key()?.ok_or(KbinError::InvalidState)?;
    let value = match base.value() {
      Ok(value) => Some(value),
      Err(e) => match e {
        KbinError::InvalidNodeType { .. } => None,
        _ => return Err(e),
      },
    };

    let mut elem = BytesStart::borrowed(key.as_bytes(), key.as_bytes().len());

    if base.is_array {
      let values = value.as_ref().ok_or(KbinError::InvalidState)?.as_array()?;

      elem.push_attribute(Attribute {
        key: b"__count",
        value: Cow::Owned(values.len().to_string().into_bytes()),
      });
    }

    if base.node_type == StandardType::Binary {
      let value = value.as_ref().ok_or(KbinError::InvalidState)?.as_slice()?;

      elem.push_attribute(Attribute {
        key: b"__size",
        value: Cow::Owned(value.len().to_string().into_bytes()),
      });
    }

    // Only add a `__type` attribute if this is not a `NodeStart` node
    if base.node_type != StandardType::NodeStart {
      elem.push_attribute(Attribute {
        key: b"__type",
        value: Cow::Borrowed(base.node_type.name.as_bytes()),
      });
    }

    for attribute in self.attributes() {
      let key = attribute.key()?.ok_or(KbinError::InvalidState)?.into_bytes();
      let value = attribute.value()?.to_string();
      let value = BytesText::from_plain_str(&value);

      elem.push_attribute(Attribute {
        key: &key,
        value: Cow::Borrowed(value.escaped()),
      });
    }

    let start_elem = match value {
      Some(value) => {
        writer.write_event(Event::Start(elem))?;

        let value = value.to_string();
        let elem = BytesText::from_plain_str(&value);
        writer.write_event(Event::Text(elem))?;

        None
      },
      None => Some(elem),
    };

    let has_value = start_elem.is_none();
    let has_children = !self.children().is_empty();

    // A `Some` value here means the start element was not written
    if let Some(start_elem) = start_elem {
      if !has_children {
        writer.write_event(Event::Empty(start_elem))?;
      } else {
        writer.write_event(Event::Start(start_elem))?;
      }
    }

    for child in self.children() {
      child.write(writer)?;
    }

    if has_value || has_children {
      let end_elem = BytesEnd::borrowed(key.as_bytes());
      writer.write_event(Event::End(end_elem))?;
    }

    Ok(())
  }
}
