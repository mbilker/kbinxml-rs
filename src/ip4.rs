use std::fmt;
use std::net::Ipv4Addr;
use std::ops::{Deref, DerefMut};

use serde::de::{Deserialize, Deserializer, SeqAccess, Visitor};
use serde::ser::{Serialize, SerializeTupleStruct, Serializer};

pub struct Ip4Addr(Ipv4Addr);

struct Ip4Visitor;

impl Ip4Addr {
  pub fn new(addr: Ipv4Addr) -> Self {
    Ip4Addr(addr)
  }
}

impl fmt::Display for Ip4Addr {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    fmt::Display::fmt(&self.0, f)
  }
}

impl fmt::Debug for Ip4Addr {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    fmt::Debug::fmt(&self.0, f)
  }
}

impl Deref for Ip4Addr {
  type Target = Ipv4Addr;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for Ip4Addr {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl<'de> Visitor<'de> for Ip4Visitor {
  type Value = [u8; 4];

  fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.write_str("a sequence of 4 bytes with no size indicator")
  }

  fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where A: SeqAccess<'de>
  {
    trace!("Ip4Addr::visit_seq()");
    let v1: u8 = seq.next_element()?.unwrap();
    let v2: u8 = seq.next_element()?.unwrap();
    let v3: u8 = seq.next_element()?.unwrap();
    let v4: u8 = seq.next_element()?.unwrap();
    trace!("Ip4Addr:visit_seq() => [{}, {}, {}, {}]", v1, v2, v3, v4);
    Ok([v1, v2, v3, v4])
  }
}

impl<'de> Deserialize<'de> for Ip4Addr {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: Deserializer<'de>
  {
    deserializer.deserialize_tuple_struct("ip4", 4, Ip4Visitor)
      .map(|v| {
        Ip4Addr(Ipv4Addr::from(v))
      })
  }
}

impl Serialize for Ip4Addr {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer
  {
    let octets = self.0.octets();
    let mut ts = serializer.serialize_tuple_struct("ip4", 4)?;
    ts.serialize_field(&octets[0])?;
    ts.serialize_field(&octets[1])?;
    ts.serialize_field(&octets[2])?;
    ts.serialize_field(&octets[3])?;
    ts.end()
  }
}
