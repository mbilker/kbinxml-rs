use serde::de::{DeserializeSeed, SeqAccess};

use de::{Deserializer, Result};
use error::Error;

pub struct Seq<'a, 'de: 'a> {
  de: &'a mut Deserializer<'de>,
  index: usize,
  len: usize,
}

impl<'de, 'a> Seq<'a, 'de> {
  pub fn new(de: &'a mut Deserializer<'de>, len: Option<usize>) -> Self {
    let len = len.unwrap_or(0);
    Self {
      de,
      index: 0,
      len,
    }
  }
}

impl<'de, 'a> SeqAccess<'de> for Seq<'a, 'de> {
  type Error = Error;

  fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where T: DeserializeSeed<'de>
  {
    if self.index >= self.len {
      return Ok(None);
    }
    self.index += 1;

    seed.deserialize(&mut *self.de).map(Some)
  }
}
