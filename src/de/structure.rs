use std::marker::PhantomData;

use serde::de::{DeserializeSeed, MapAccess};

use de::{Deserializer, ReadMode, Result};
use error::{Error, KbinErrorKind};
use node_types::StandardType;

pub struct Struct<'a, 'de: 'a> {
  de: &'a mut Deserializer<'de>,
  values_to_consume: usize,
}

impl<'de, 'a> Struct<'a, 'de> {
  pub fn new(de: &'a mut Deserializer<'de>) -> Self {
    trace!("--> Struct::new()");

    Self {
      de,
      values_to_consume: 0,
    }
  }
}

impl<'de, 'a> MapAccess<'de> for Struct<'a, 'de> {
  type Error = Error;

  fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where K: DeserializeSeed<'de>
  {
    trace!("--> <Struct as MapAccess>::next_key_seed()");

    let (node_type, is_array) = self.de.reader.read_node_type()?;
    debug!("Struct::next_key_seed() => node_type: {:?}", node_type);

    match node_type {
      StandardType::NodeEnd |
      StandardType::FileEnd => {
        debug!("<-- <Struct as MapAccess>::next_key_seed() => end of map");
        return Ok(None);
      },
      _ => {},
    };

    let old_read_mode = self.de.set_read_mode(ReadMode::Key);
    let key = seed.deserialize(&mut *self.de).map(Some)?;
    self.de.read_mode = old_read_mode;

    match node_type {
      StandardType::NodeStart => {
        debug!("Struct::next_key_seed() => got a node start!");
      },
      StandardType::Attribute => {
        debug!("Struct::next_key_seed() => got an attribute!");
      },
      _ => {
        // TODO(mbilker): Fix processing of `Attribute` nodes for non-NodeStart
        // elements
        loop {
          let (node_type, _is_array) = self.de.reader.peek_node_type()?;
          if node_type == StandardType::Attribute {
            warn!("Struct::next_key_seed() => ignoring Attribute node");
            let key: Option<String> = self.next_key_seed(PhantomData)?;
            warn!("Struct::next_key_seed() => ignored Attribute key: {:?}", key);

            self.values_to_consume += 1;
          } else {
            break;
          }
        }

        // Consume the end node and do a sanity check
        let (node_type, _is_array) = self.de.reader.read_node_type()?;
        if node_type != StandardType::NodeEnd {
          return Err(KbinErrorKind::TypeMismatch(*StandardType::NodeEnd, *node_type).into());
        }
      },
    }

    // Store the current node type on the stack for stateful handling based on
    // the current node type
    self.de.node_stack.push((node_type, is_array));

    Ok(key)
  }

  fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where V: DeserializeSeed<'de>
  {
    debug!("--> <Struct as MapAccess>::next_value_seed()");
    let value = seed.deserialize(&mut *self.de)?;

    for _ in 0..self.values_to_consume {
      warn!("Struct::next_value_seed() => ignoring Attribute node value");
      let seed = PhantomData;
      let value: String = seed.deserialize(&mut *self.de)?;
      warn!("Struct::next_value_seed() => ignored Attribute value: {:?}", value);

      let popped = self.de.node_stack.pop();
      debug!("<Struct as MapAccess>::next_value_seed() => popped: {:?}, node_stack: {:?}", popped, self.de.node_stack);
    }
    self.values_to_consume = 0;

    let popped = self.de.node_stack.pop();
    debug!("<Struct as MapAccess>::next_value_seed() => popped: {:?}, node_stack: {:?}", popped, self.de.node_stack);

    Ok(value)
  }
}
