use serde::ser::Serialize;

use node::Node;

impl Serialize for Node {
  #[inline]
  fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where S: ::serde::Serializer
  {
    unimplemented!();
  }
}
