use serde::de::{DeserializeSeed, SeqAccess};

use de::{Deserializer, Result};
use error::{Error, KbinErrorKind};
use node_types::StandardType;

pub struct Seq<'a, 'de: 'a> {
  de: &'a mut Deserializer<'de>,
  index: usize,
  len: Option<usize>,
  known_identifier: Option<String>,
}

impl<'de, 'a> Seq<'a, 'de> {
  pub fn new(de: &'a mut Deserializer<'de>, len: Option<usize>) -> Result<Self> {
    trace!("Seq::new(len: {:?})", len);

    let known_identifier = if len.is_none() {
      let value = de.reader.last_identifier().ok_or(KbinErrorKind::InvalidState)?.into();
      debug!("Seq::new(len: {:?}) => known identifier: {:?}", len, value);

      Some(value)
    } else {
      None
    };

    Ok(Self {
      de,
      index: 0,
      len,
      known_identifier,
    })
  }

  fn is_end(&mut self, node_type: StandardType) -> Result<bool> {
    // The struct sequence ends when the node identifier has a different name
    // and the current node type is not `NodeStart` or `NodeEnd` and the last
    // node was not a `NodeStart` event.
    //
    // The should not trigger for struct subfields because those would be
    // deserialized by the struct deserializer.
    if node_type != StandardType::NodeStart &&
       node_type != StandardType::NodeEnd
    {
      let node_identifier = self.de.reader.peek_node_identifier()?;
      let known_identifier = self.known_identifier.as_ref().ok_or(KbinErrorKind::InvalidState)?;
      if node_identifier.as_str() != known_identifier {
        debug!("Seq::is_end() => peeked identifier does not equal known identifier: {:?}", known_identifier);
        return Ok(true);
      }
    }

    Ok(false)
  }
}

impl<'de, 'a> SeqAccess<'de> for Seq<'a, 'de> {
  type Error = Error;

  // A len of `None` indicates that the sequence ends when `NodeEnd` is reached
  // or a different type node is reached.
  fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where T: DeserializeSeed<'de>
  {
    trace!("--> Seq::next_element_seed()");

    if let Some(len) = self.len {
      if self.index >= len {
        debug!("Seq::next_element_seed() => out of bounds read, returning None");

        return Ok(None);
      }
    } else {
      let (node_type, _is_array) = self.de.reader.peek_node_type()?;
      let (last_node_type, _is_array) = self.de.reader.last_node_type().ok_or(KbinErrorKind::InvalidState)?;
      debug!("Seq::next_element_seed() => peeked type: {:?}, last type: {:?}", node_type, self.de.reader.last_node_type());

      if self.is_end(last_node_type)? {
        debug!("<-- Seq::next_element_seed() => end of sequence (by last read node)");
        return Ok(None);
      }

      // If the peeked node is not a `NodeStart` and this isn't the first
      // element in the list, check the identifier
      if self.index > 0 {
        if self.is_end(node_type)? {
          debug!("<-- Seq::next_element_seed() => end of sequence (by peeked node)");
          return Ok(None);
        }
      }

      // `NodeEnd` signals the end of the sequence for a sequence of structs
      // if all the structs in the sequence have the same type
      match node_type {
        // Trigger `deserialize_struct` to consume the `NodeStart` event after
        // the first element in the struct sequence
        StandardType::NodeStart => self.de.first_struct = true,
        StandardType::NodeEnd => return Ok(None),
        _ => {},
      };
    }
    self.index += 1;

    seed.deserialize(&mut *self.de).map(Some)
  }
}
