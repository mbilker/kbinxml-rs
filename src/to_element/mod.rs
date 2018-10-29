use minidom::Element;

mod node;

pub trait ToElement {
  fn to_element(&self) -> Element;
}
