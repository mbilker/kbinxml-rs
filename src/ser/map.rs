use byteorder::WriteBytesExt;
use failure::ResultExt;
use serde::ser::{Serialize, SerializeMap};

use error::{Error, KbinErrorKind};
use node_types::StandardType;
use ser::{Result, Serializer, TypeHint, WriteMode, ARRAY_MASK};
use sixbit::pack_sixbit;

pub struct Map<'a> {
  ser: &'a mut Serializer,
}

impl<'a> Map<'a> {
  pub fn new(ser: &'a mut Serializer) -> Result<Self> {
    debug!("Map::new()");

    // Restrict bounds of immutable borrow from `hierarchy` Vec
    {
      // The key name would have been pushed to the stack in
      // `<Struct as SerializeStruct>::serialize_field` before calling
      // `serialize` on the value
      let name = if let Some(key) = ser.hierarchy.last() {
        trace!("Map::new() => found key name: {}", key);
        key
      } else {
        return Err(KbinErrorKind::InvalidState.into());
      };

      let node_type = StandardType::NodeStart;
      ser.node_buf.write_u8(node_type.id).context(KbinErrorKind::DataWrite(node_type.name))?;
      pack_sixbit(&mut *ser.node_buf, name)?;
    }

    Ok(Self { ser })
  }
}

impl<'a> SerializeMap for Map<'a> {
  type Ok = Option<TypeHint>;
  type Error = Error;

  fn serialize_entry<K, V>(&mut self, key: &K, value: &V) -> Result<()>
    where K: ?Sized + Serialize,
          V: ?Sized + Serialize
  {
    trace!("--> <Map as SerializeMap>::serialize_entry()");

    // Serialize methods that return `None` will not be written
    if let Some(hint) = value.serialize(&mut *self.ser)? {
      let node_type = hint.node_type;
      debug!("SerializeMap::serialize_entry() => hint: {:?}", hint);

      // Struct handler outputs `NodeStart` event by itself. Avoid repeating it.
      if node_type != StandardType::NodeStart {
        self.ser.write_node(hint)?;
        self.serialize_key(key)?;

        if node_type != StandardType::Attribute {
          self.ser.node_buf.write_u8(StandardType::NodeEnd.id | ARRAY_MASK).context(KbinErrorKind::DataWrite("node end"))?;
        }
      }
    }

    Ok(())
  }

  fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where T: ?Sized + Serialize
  {
    trace!("--> <Map as SerializeMap>::serialize_key()");

    self.ser.write_mode = WriteMode::Identifier;
    let hint = key.serialize(&mut *self.ser)?.ok_or(KbinErrorKind::MissingTypeHint)?;
    debug!("<Map as SerializeMap>::serialize_key() => hint: {:?}", hint);

    self.ser.write_mode = WriteMode::Single;

    match hint.node_type {
      StandardType::Attribute |
      StandardType::String => Ok(()),
      node_type => Err(KbinErrorKind::TypeMismatch(*StandardType::String, *node_type).into()),
    }
  }

  fn serialize_value<T>(&mut self, _value: &T) -> Result<()>
    where T: ?Sized + Serialize
  {
    trace!("--> <Map as SerializeMap>::serialize_value()");
    unimplemented!();
  }

  fn end(self) -> Result<Self::Ok> {
    trace!("<Map as SerializeMap>::end()");
    self.ser.node_buf.write_u8(StandardType::NodeEnd.id | ARRAY_MASK).context(KbinErrorKind::DataWrite("node end"))?;

    Ok(Some(TypeHint::from_type(StandardType::NodeStart)))
  }
}
