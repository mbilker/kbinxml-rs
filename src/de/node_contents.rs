use serde::de::{DeserializeSeed, IntoDeserializer, MapAccess};

use de::{Custom, Deserializer, ReadMode, Result};
use error::{Error, KbinErrorKind};
use node_types::StandardType;

#[derive(Debug)]
enum ReadState {
  Value,
  Attributes,
}

pub struct NodeContents<'a, 'de: 'a> {
  de: &'a mut Deserializer<'de>,
  node_type: StandardType,
  state: ReadState,
}

impl<'de, 'a> NodeContents<'a, 'de> {
  pub fn new(de: &'a mut Deserializer<'de>, node_type: StandardType) -> Self {
    trace!("--> NodeContents::new()");

    Self {
      de,
      node_type,
      state: ReadState::Value,
    }
  }
}

impl<'de, 'a> MapAccess<'de> for NodeContents<'a, 'de> {
  type Error = Error;

  fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where K: DeserializeSeed<'de>
  {
    trace!("--> <NodeContents as MapAccess>::next_key_seed(state: {:?})", self.state);

    match self.state {
      ReadState::Value => {
        let de = "__value".into_deserializer();
        seed.deserialize(Custom::new(de, self.node_type)).map(Some)
      },
      ReadState::Attributes => {
        let (node_type, _is_array) = self.de.reader.read_node_type()?;
        debug!("NodeContents::next_key_seed() => node_type: {:?}", node_type);

        match node_type {
          StandardType::Attribute => {},
          StandardType::NodeEnd |
          StandardType::FileEnd => {
            debug!("<-- <NodeContents as MapAccess>::next_key_seed() => end of map, node stack: {:?}", self.de.node_stack);

            return Ok(None);
          },
          _ => return Err(KbinErrorKind::InvalidState.into()),
        };

        let old_read_mode = self.de.set_read_mode(ReadMode::Key);
        let key = seed.deserialize(&mut *self.de).map(Some)?;
        self.de.read_mode = old_read_mode;

        self.node_type = node_type;

        Ok(key)
      },
    }
  }

  fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where V: DeserializeSeed<'de>
  {
    trace!("--> <NodeContents as MapAccess>::next_value_seed(state: {:?})", self.state);

    match self.state {
      ReadState::Value => {
        let value = seed.deserialize(&mut *self.de)?;
        self.state = ReadState::Attributes;

        Ok(value)
      },
      ReadState::Attributes => {
        seed.deserialize(Custom::new(&mut *self.de, self.node_type))
      },
    }
  }
}
