use serde::de::{DeserializeSeed, SeqAccess};

use de::{Deserializer, Result};
use error::Error;

pub struct Seq<'a, 'de: 'a> {
  de: &'a mut Deserializer<'de>,
  index: usize,
  len: usize,
}

impl<'de, 'a> Seq<'a, 'de> {
  pub fn new(de: &'a mut Deserializer<'de>, len: usize) -> Self {
    trace!("Seq::new(len: {})", len);

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
      trace!("Seq::next_element_seed() => out of bounds read, returning None");

      return Ok(None);
    }
    self.index += 1;

    seed.deserialize(&mut *self.de).map(Some)
  }
}
