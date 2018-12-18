use serde::ser::{Serialize, SerializeTupleStruct};

use crate::error::KbinErrorKind;
use crate::node_types::StandardType;
use crate::ser::{Error, Result, Serializer, TypeHint, WriteMode};

pub struct Custom<'a> {
  ser: &'a mut Serializer,
  node_type: StandardType,
}

impl<'a> Custom<'a> {
  pub fn new(ser: &'a mut Serializer, name: &'static str, len: usize) -> Result<Self> {
    let node_type = StandardType::from_name(name);

    // Custom node type handler
    //
    // Sets the serializer to output a specific format for wrapper types
    match node_type {
      StandardType::Ip4 => ser.write_mode = WriteMode::Array,
      _ => {},
    };

    // TODO: Fix check for types that have a count above 1
    let node_size = node_type.size as usize;
    if node_size != len {
      return Err(KbinErrorKind::SizeMismatch(*node_type, node_size, len).into());
    }

    Ok(Self { ser, node_type })
  }
}

impl<'a> SerializeTupleStruct for Custom<'a> {
  type Ok = Option<TypeHint>;
  type Error = Error;

  fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where T: ?Sized + Serialize
  {
    trace!("Custom::serialize_field()");

    value.serialize(&mut *self.ser)?;

    Ok(())
  }

  fn end(self) -> Result<Self::Ok> {
    trace!("Custom::end()");

    self.ser.write_mode = WriteMode::Single;

    Ok(Some(TypeHint::from_type(self.node_type)))
  }
}
