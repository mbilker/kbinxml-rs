use minidom::Element;

use node::Node;
use value::Value;

pub trait ToElement {
  fn to_element(&self) -> Element;
}

impl ToElement for Node {
  fn to_element(&self) -> Element {
    let mut elem = Element::bare(self.key());

    if let Some(value) = self.value() {
      elem.set_attr("__type", value.standard_type().name);

      match value {
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
