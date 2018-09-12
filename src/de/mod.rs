use std::result::Result as StdResult;

use bytes::Bytes;
use serde::de::{self, Deserialize, Visitor};

use error::{Error, KbinErrorKind};
use node::NodeCollection;
use reader::Reader;

mod collection;
mod custom;
mod definition;
mod node_contents;
mod seq;
mod structure;

use self::custom::Custom;
use self::structure::Struct;

pub type Result<T> = StdResult<T, Error>;

pub struct Deserializer {
  collection: NodeCollection,
}

pub fn from_bytes<'a, T>(input: &'a [u8]) -> Result<T>
  where T: Deserialize<'a>
{
  let mut deserializer = Deserializer::new(input)?;
  let t = T::deserialize(&mut deserializer)?;
  Ok(t)
}

impl Deserializer {
  pub fn new(input: &[u8]) -> Result<Self> {
    let mut reader = Reader::new(Bytes::from(input))?;
    let collection = NodeCollection::from_iter(&mut reader).ok_or(KbinErrorKind::InvalidState)?;

    Ok(Self { collection })
  }
}

impl<'de, 'a> de::Deserializer<'de> for &'a mut Deserializer {
  type Error = Error;

  fn is_human_readable(&self) -> bool {
    false
  }

  fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_any()");
    self.deserialize_map(visitor)
  }

  forward_to_deserialize_any! {
    bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str
    string bytes byte_buf option unit unit_struct newtype_struct seq
    tuple tuple_struct enum identifier
  }

  fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_map()");

    let value = visitor.visit_map(Struct::new(&mut self.collection))?;
    let keys: Vec<_> = self.collection.children().iter()
      .filter_map(|x| x.base().key().ok())
      .collect();
    trace!("Deserializer::deserialize_map() => end, keys: {:?}", keys);

    Ok(value)
  }

  fn deserialize_struct<V>(self, name: &'static str, fields: &'static [&'static str], visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_struct(name: {:?}, fields: {:?})", name, fields);

    let value = visitor.visit_map(Struct::new(&mut self.collection))?;
    let keys: Vec<_> = self.collection.children().iter()
      .filter_map(|x| x.base().key().ok())
      .collect();
    trace!("Deserializer::deserialize_struct(name: {:?}) => end, keys: {:?}", name, keys);

    Ok(value)
  }

  fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    trace!("Deserializer::deserialize_ignored_any()");
    self.deserialize_any(visitor)
  }
}
