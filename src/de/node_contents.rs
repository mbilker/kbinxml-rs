use serde::de::{DeserializeSeed, IntoDeserializer, MapAccess};

use de::{Custom, Result};
use de::definition::NodeDefinitionDeserializer;
use error::{Error, KbinErrorKind};
use node::NodeCollection;

#[derive(Debug)]
enum ReadState {
  Value,
  Attributes,
}

pub struct NodeContents<'a, 'de: 'a> {
  collection: &'a mut NodeCollection<'de>,
  state: ReadState,
}

impl<'de, 'a> NodeContents<'a, 'de> {
  pub fn new(collection: &'a mut NodeCollection<'de>) -> Self {
    trace!("--> NodeContents::new(node_type: {:?})", collection.base().node_type);

    Self {
      collection,
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
        let base = self.collection.base();
        let de = "__value".into_deserializer();
        seed.deserialize(Custom::new(de, base.node_type)).map(Some)
      },
      ReadState::Attributes => {
        if let Some(attribute) = self.collection.attributes().front() {
          let key = attribute.key()?.ok_or(KbinErrorKind::InvalidState)?;
          debug!("<NodeContents as MapAccess>::next_key_seed(state: {:?}) => attribute: {:?}, key: {:?}", self.state, attribute, key);

          let de = NodeDefinitionDeserializer::new(*attribute);
          seed.deserialize(de).map(Some)
        } else {
          debug!("<-- <NodeContents as MapAccess>::next_key_seed(state: {:?}) => end of map", self.state);

          Ok(None)
        }
      },
    }
  }

  fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where V: DeserializeSeed<'de>
  {
    trace!("--> <NodeContents as MapAccess>::next_value_seed(state: {:?})", self.state);

    match self.state {
      ReadState::Value => {
        let base = self.collection.base();
        let de = NodeDefinitionDeserializer::new(base);
        let value = seed.deserialize(de)?;
        self.state = ReadState::Attributes;

        Ok(value)
      },
      ReadState::Attributes => {
        if let Some(attribute) = self.collection.attributes_mut().pop_front() {
          let node_type = attribute.node_type;
          let value = attribute.value()?;
          debug!("<NodeContents as MapAccess>::next_value_seed() => attribute: {:?}, value: {:?}", attribute, value);

          let de = value.into_deserializer();
          seed.deserialize(Custom::new(de, node_type))
        } else {
          Err(KbinErrorKind::InvalidState.into())
        }
      },
    }
  }
}
