use std::fmt::Write;

use minidom::Element;

use node::Node;
use to_element::ToElement;
use value::Value;

impl ToElement for Node {
  fn to_element(&self) -> Element {
    let mut elem = Element::bare(self.key());

    if let Some(value) = self.value() {
      elem.set_attr("__type", value.standard_type().name);

      match value {
        Value::Binary(data) => {
          elem.set_attr("__size", data.len());

          let len = data.len() * 2;
          let value = data.into_iter().fold(String::with_capacity(len), |mut val, x| {
            write!(val, "{:02x}", x).expect("Failed to append hex char");
            val
          });
          elem.append_text_node(value);
        },
        Value::String(value) => {
          elem.append_text_node(value.as_str());
        },
        Value::Array(_, values) => {
          elem.set_attr("__count", values.len());

          let value = Value::array_as_string(values);
          elem.append_text_node(value);
        },
        value => {
          let value = value.to_string();
          elem.append_text_node(value);
        },
      }
    }

    if let Some(attributes) = self.attributes() {
      for (key, value) in attributes {
        elem.set_attr(key.as_str(), value.as_str());
      }
    }

    if let Some(children) = self.children() {
      for child in children {
        let child = child.to_element();
        elem.append_child(child);
      }
    }

    elem
  }
}
