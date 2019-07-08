use std::borrow::Cow;
use std::io::Write;

use quick_xml::Writer;
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::events::attributes::Attribute;

use crate::encoding_type::EncodingType;
use crate::error::KbinError;
use crate::node::Node;
use crate::node_types::StandardType;
use crate::to_text_xml::ToTextXml;
use crate::value::Value;

impl ToTextXml for Node {
  /// At the moment, a `Node` will always contain UTF-8 data.
  fn encoding(&self) -> EncodingType {
    EncodingType::UTF_8
  }

  fn write<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), KbinError> {
    let key = self.key();
    let mut elem = BytesStart::borrowed(key.as_bytes(), key.as_bytes().len());

    // Write the attributes for the value, but not the value contents.
    if let Some(value) = self.value() {
      let node_type = value.standard_type();

      match value {
        Value::Binary(ref data) => {
          elem.push_attribute(Attribute {
            key: b"__size",
            value: Cow::Owned(data.len().to_string().into_bytes()),
          });
        },
        Value::Array(_, ref values) => {
          elem.push_attribute(Attribute {
            key: b"__count",
            value: Cow::Owned(values.len().to_string().into_bytes()),
          });
        },
        Value::ArrayNew(ref values) => {
          elem.push_attribute(Attribute {
            key: b"__count",
            value: Cow::Owned(values.len().to_string().into_bytes()),
          });
        },
        _ => {},
      };

      // Only add a `__type` attribute if this is not a `NodeStart` node
      if node_type != StandardType::NodeStart {
        elem.push_attribute(Attribute {
          key: b"__type",
          value: Cow::Borrowed(node_type.name.as_bytes()),
        });
      }
    }

    if let Some(attributes) = self.attributes() {
      for (key, value) in attributes {
        let value = BytesText::from_plain_str(&value);

        elem.push_attribute(Attribute {
          key: key.as_bytes(),
          value: Cow::Borrowed(value.escaped()),
        });
      }
    }

    // Now write the value contents.
    let start_elem = if let Some(value) = self.value() {
      writer.write_event(Event::Start(elem))?;

      let value = value.to_string();
      let elem = BytesText::from_plain_str(&value);
      writer.write_event(Event::Text(elem))?;

      None
    } else {
      Some(elem)
    };

    let has_value = start_elem.is_none();
    let has_children = match self.children() {
      Some(children) => !children.is_empty(),
      None => false,
    };

    // A `Some` value here means the start element was not written
    if let Some(start_elem) = start_elem {
      if !has_children {
        writer.write_event(Event::Empty(start_elem))?;
      } else {
        writer.write_event(Event::Start(start_elem))?;
      }
    }

    if let Some(children) = self.children() {
      for child in children {
        child.write(writer)?;
      }
    }

    if has_value || has_children {
      let end_elem = BytesEnd::borrowed(key.as_bytes());
      writer.write_event(Event::End(end_elem))?;
    }

    Ok(())
  }
}
