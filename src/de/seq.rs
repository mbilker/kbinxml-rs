use std::collections::VecDeque;

use serde::de::{DeserializeSeed, IntoDeserializer, SeqAccess};

use de::collection::NodeCollectionDeserializer;
use error::{Error, KbinErrorKind};
use node::NodeCollection;
use node_types::StandardType;
use value::Value;

enum SequenceMode {
  Struct {
    known_identifier: String,
  },
  Value {
    values: VecDeque<Value>,
  },
}

pub struct Seq<'a, 'de: 'a> {
  collection: &'a mut NodeCollection<'de>,
  index: usize,
  seq_mode: SequenceMode,
}

impl<'de, 'a> Seq<'a, 'de> {
  pub fn new(collection: &'a mut NodeCollection<'de>, is_array: bool) -> Result<Self, Error> {
    trace!("Seq::new(is_array: {})", is_array);

    let seq_mode = if is_array {
      let base = collection.base();
      let value = base.value()?;
      let values = if let Value::Array(node_type, values) = value {
        debug!("Seq::new(is_array: {}) => len: {}", is_array, values.len());

        if node_type != base.node_type {
          return Err(KbinErrorKind::TypeMismatch(base.node_type, node_type).into());
        }

        VecDeque::from(values)
      } else {
        return Err(KbinErrorKind::InvalidState.into());
      };

      SequenceMode::Value { values }
    } else {
      let child = collection.children().front().ok_or(KbinErrorKind::InvalidState)?;
      let known_identifier = child.base().key()?.ok_or(KbinErrorKind::InvalidState)?;
      debug!("Seq::new(is_array: {}) => known identifier: {:?}", is_array, known_identifier);

      SequenceMode::Struct { known_identifier }
    };

    Ok(Self {
      collection,
      index: 0,
      seq_mode,
    })
  }
}

impl<'de, 'a> SeqAccess<'de> for Seq<'a, 'de> {
  type Error = Error;

  // A len of `None` indicates that the sequence ends when `NodeEnd` is reached
  // or a different type node is reached.
  fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where T: DeserializeSeed<'de>
  {
    trace!("--> Seq::next_element_seed()");

    match self.seq_mode {
      SequenceMode::Struct { ref known_identifier } => {
        let base = match self.collection.children().front() {
          Some(child) => child.base(),
          None => {
            debug!("<-- Seq::next_element_seed(mode: Struct) => end of sequence (by `collection.children.front() == None`)");
            return Ok(None);
          },
        };

        let node_type = base.node_type;
        debug!("Seq::next_element_seed(mode: Struct) => peeked type: {:?}", node_type);

        if self.index > 0 {
          // The struct sequence ends when the node identifier has a different name
          // and the current node type is not `NodeStart` or `NodeEnd` and the last
          // node was not a `NodeStart` event.
          //
          // The should not trigger for struct subfields because those would be
          // deserialized by the struct deserializer.
          if node_type != StandardType::NodeStart &&
             node_type != StandardType::NodeEnd
          {
            let node_identifier = base.key()?.ok_or(KbinErrorKind::InvalidState)?;
            if node_identifier.as_str() != known_identifier {
              debug!("<-- Seq::next_element_seed(mode: Struct) => peeked identifier does not equal known identifier: {:?}", known_identifier);
              return Ok(None);
            }
          }
        }
        self.index += 1;

        let de = NodeCollectionDeserializer::new(&mut self.collection);
        seed.deserialize(de).map(Some)
      },
      SequenceMode::Value { ref mut values } => {
        let value = match values.pop_front() {
          Some(v) => v,
          None => {
            debug!("<-- Seq::next_element_seed(mode: Value) => out of bounds read, returning None");

            return Ok(None);
          },
        };

        let de = value.into_deserializer();
        seed.deserialize(de).map(Some)
      },
    }
  }
}
