use std::io::{Seek, SeekFrom};

use byteorder::WriteBytesExt;
use failure::ResultExt;
use serde::ser::{Serialize, SerializeMap};

use error::{Error, KbinErrorKind};
use node_types::StandardType;
use ser::{Result, Serializer, TypeHint, WriteMode, ARRAY_MASK};

pub struct Map<'a> {
  ser: &'a mut Serializer,

  current_node_index: u64,

  key_node_type: StandardType,
}

impl<'a> Map<'a> {
  pub fn new(ser: &'a mut Serializer) -> Result<Self> {
    debug!("Map::new()");

    ser.write_node(TypeHint::from_type(StandardType::NodeStart))?;
    ser.write_identifier("something")?;

    Ok(Self {
      ser,
      current_node_index: 0,
      key_node_type: StandardType::NodeStart,
    })
  }
}

impl<'a> SerializeMap for Map<'a> {
  type Ok = Option<TypeHint>;
  type Error = Error;

  fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where T: ?Sized + Serialize
  {
    trace!("--> <Map as SerializeMap>::serialize_key()");

    self.current_node_index = self.ser.node_buf.position();
    self.ser.node_buf.write_u8(0).context(KbinErrorKind::DataWrite("placeholder"))?;

    self.ser.write_mode = WriteMode::Identifier;
    let hint = key.serialize(&mut *self.ser)?.ok_or(KbinErrorKind::MissingTypeHint)?;
    debug!("<Map as SerializeMap>::serialize_key() => hint: {:?}", hint);

    self.ser.write_mode = WriteMode::Single;

    self.key_node_type = hint.node_type;
    match hint.node_type {
      StandardType::Attribute |
      StandardType::String => Ok(()),
      node_type => return Err(KbinErrorKind::TypeMismatch(*StandardType::String, *node_type).into()),
    }
  }

  fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where T: ?Sized + Serialize
  {
    trace!("--> <Map as SerializeMap>::serialize_value()");

    let mut hint = value.serialize(&mut *self.ser)?.ok_or(KbinErrorKind::MissingTypeHint)?;
    debug!("<Map as SerializeMap>::serialize_value() => hint: {:?}", hint);

    // Attributes must have a string body
    if self.key_node_type == StandardType::Attribute {
      if hint.node_type == StandardType::String {
        hint = TypeHint::from_type(StandardType::Attribute);
      } else {
        return Err(KbinErrorKind::TypeMismatch(*StandardType::Attribute, *hint.node_type).into());
      }
    }

    let new_pos = self.ser.node_buf.position();
    self.ser.node_buf.seek(SeekFrom::Start(self.current_node_index)).context(KbinErrorKind::Seek)?;
    self.ser.write_node(hint)?;
    self.ser.node_buf.seek(SeekFrom::Start(new_pos)).context(KbinErrorKind::Seek)?;

    if self.key_node_type != StandardType::Attribute {
      self.ser.node_buf.write_u8(StandardType::NodeEnd.id | ARRAY_MASK).context(KbinErrorKind::DataWrite("node end"))?;
    }

    Ok(())
  }

  fn end(self) -> Result<Self::Ok> {
    trace!("<Map as SerializeMap>::end()");
    self.ser.node_buf.write_u8(StandardType::NodeEnd.id | ARRAY_MASK).context(KbinErrorKind::DataWrite("node end"))?;

    Ok(None)
  }
}
