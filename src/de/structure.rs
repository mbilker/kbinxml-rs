use serde::de::{DeserializeSeed, MapAccess};

use de::{Deserializer, Result};
use error::{Error, KbinErrorKind};
use node_types::StandardType;

pub struct Struct<'a, 'de: 'a> {
  de: &'a mut Deserializer<'de>,
  //fields: &'static [&'static str],
}

impl<'de, 'a> Struct<'a, 'de> {
  pub fn new(de: &'a mut Deserializer<'de>, _fields: &'static [&'static str]) -> Self {
    Self {
      de,
      //fields,
    }
  }
}

impl<'de, 'a> MapAccess<'de> for Struct<'a, 'de> {
  type Error = Error;

  fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where K: DeserializeSeed<'de>
  {
    trace!("--> <Struct as MapAccess>::next_key_seed()");

    let (node_type, _is_array) = self.de.reader.read_node_type()?;
    debug!("Struct::next_key_seed() => node_type: {:?}", node_type);

    if node_type == StandardType::NodeEnd {
      trace!("Struct::next_key_seed() => end of map");
      return Ok(None);
    }

    let key = seed.deserialize(&mut *self.de).map(Some)?;

    match node_type {
      StandardType::NodeStart => {
        debug!("Struct::next_key_seed() => got a node start!");
      },
      StandardType::Attribute => {
        debug!("Struct::next_key_seed() => got an attribute!");
      },
      _ => {
        // Consume the end node and do a sanity check
        let (node_type, _is_array) = self.de.reader.read_node_type()?;
        if node_type != StandardType::NodeEnd {
          return Err(KbinErrorKind::TypeMismatch(*StandardType::NodeEnd, *node_type).into());
        }
      },
    }

    // Store the current node type on the stack for stateful handling based on
    // the current node type
    self.de.node_stack.push(node_type);

    Ok(key)
  }

  fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where V: DeserializeSeed<'de>
  {
    debug!("--> <Struct as MapAccess>::next_value_seed()");
    let value = seed.deserialize(&mut *self.de)?;

    let popped = self.de.node_stack.pop();
    debug!("<Struct as MapAccess>::next_value_seed() => popped: {:?}, node_stack: {:?}", popped, self.de.node_stack);

    Ok(value)
  }
}
