use serde::de::{DeserializeSeed, IntoDeserializer, MapAccess};

use de::Custom;
use de::collection::NodeCollectionDeserializer;
use de::definition::NodeDefinitionDeserializer;
use error::Error;
use node::NodeCollection;
use node_types::StandardType;

pub struct Struct<'a, 'de: 'a> {
  collection: &'a mut NodeCollection<'de>,
  key: Option<String>,
}

impl<'de, 'a> Struct<'a, 'de> {
  pub fn new(collection: &'a mut NodeCollection<'de>) -> Self {
    let key = collection.base().key().ok().and_then(|v| v);

    trace!("--> Struct::new() => attributes len: {}, children len: {}, base: {}",
      collection.attributes().len(),
      collection.children().len(),
      collection.base());

    Self {
      collection,
      key,
    }
  }
}

impl<'de, 'a> MapAccess<'de> for Struct<'a, 'de> {
  type Error = Error;

  fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where K: DeserializeSeed<'de>
  {
    debug!("--> <Struct as MapAccess>::next_key_seed()");

    // First, if the key field is still present, emit the `__node_key` first
    if self.key.is_some() {
      let de = "__node_key".into_deserializer();
      return seed.deserialize(Custom::new(de, StandardType::String)).map(Some);
    }

    // Then if there are attributes left, deserialize them first
    if let Some(attribute) = self.collection.attributes().front() {
      let de = NodeDefinitionDeserializer::new(*attribute);
      return seed.deserialize(de).map(Some);
    }

    // Else, deserialize the child nodes
    let mut node = match self.collection.children_mut().front_mut() {
      Some(v) => v,
      None => {
        debug!("<-- <Struct as MapAccess>::next_key_seed() => end of map");
        return Ok(None);
      },
    };

    let de = NodeCollectionDeserializer::new(&mut node);
    seed.deserialize(de).map(Some)
  }

  fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where V: DeserializeSeed<'de>
  {
    debug!("--> <Struct as MapAccess>::next_value_seed()");

    // First, if the key field is still present, emit the `__node_key` value
    if let Some(key) = self.key.take() {
      let de = key.into_deserializer();
      return seed.deserialize(de);
    }

    // Then if there are attributes left, deserialize them first
    if let Some(attribute) = self.collection.attributes_mut().pop_front() {
      let de = NodeDefinitionDeserializer::new(attribute);
      return seed.deserialize(de);
    }

    // Else, deserialize the child nodes. Delegate popping nodes off the
    // children queue by the deserialize methods else handling `Struct`
    // sequences will break.
    let de = NodeCollectionDeserializer::new(&mut self.collection);
    seed.deserialize(de)
  }
}
