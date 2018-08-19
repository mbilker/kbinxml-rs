use serde::ser::{Serialize, SerializeMap};

use node::ExtraNodes;

impl Serialize for ExtraNodes {
  #[inline]
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: ::serde::Serializer
  {
    trace!("<ExtraNodes as Serialize>::serialize()");

    let len = self.attributes.len() + self.nodes.len();
    let mut map = serializer.serialize_map(Some(len))?;

    for (k, v) in &self.attributes {
      map.serialize_entry(k, v)?;
    }
    for (k, v) in &self.nodes {
      map.serialize_entry(k, v)?;
    }

    map.end()
  }
}
