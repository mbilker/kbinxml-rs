use std::io::{Seek, SeekFrom};

use byteorder::{BigEndian, WriteBytesExt};
use failure::ResultExt;
use serde::ser::{Serialize, SerializeSeq, SerializeTuple};

use error::KbinErrorKind;
use node_types::StandardType;
use ser::{Error, Result, Serializer, TypeHint, WriteMode};

pub struct Tuple<'a> {
  ser: &'a mut Serializer,

  size_index: u64,
  node_type: Option<StandardType>,
  len: usize,
}

impl<'a> Tuple<'a> {
  pub fn new(ser: &'a mut Serializer, len: usize) -> Self {
    debug!("Tuple::new(len: {})", len);

    ser.write_mode = WriteMode::Array;

    // Estimate u32 for the total size of the tuple
    let size_index = ser.data_buf.position();
    ser.data_buf.write_u32::<BigEndian>(len as u32).expect("Unable to write size placeholder");

    Self {
      ser,
      size_index,
      node_type: None,
      len,
    }
  }

  fn find_standard_type(&self) -> StandardType {
    debug!("find_standard_type => len: {}", self.len);
    self.node_type.unwrap_or(StandardType::String)
  }
}

impl<'a> SerializeTuple for Tuple<'a> {
  type Ok = Option<TypeHint>;
  type Error = Error;

  fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where T: ?Sized + Serialize
  {
    debug!("SerializeTuple: serialize_element");
    let hint = value.serialize(&mut *self.ser)?.ok_or(KbinErrorKind::MissingTypeHint)?;
    debug!("SerializeTuple: serialize_element, hint: {:?}", hint);

    // Rust tuple types can have different types per element, this is not
    // permitted by kbin
    if let Some(node_type) = self.node_type {
      if node_type != hint.node_type {
        return Err(KbinErrorKind::TypeMismatch(*node_type, *hint.node_type).into());
      }
    } else {
      self.node_type = Some(hint.node_type);
    }

    Ok(())
  }

  fn end(self) -> Result<Self::Ok> {
    debug!("SerializeTuple: end");

    self.ser.write_mode = WriteMode::Single;
    self.ser.data_buf.realign_writes(None)?;

    let node_type = self.find_standard_type();
    let size = (self.len as u32) * (node_type.size as u32);

    // Update the size estimate from the constructor
    if size as usize != self.len {
      debug!("SerializeTuple: end, size correction: {}", size);

      let current_pos = self.ser.data_buf.position();
      self.ser.data_buf.seek(SeekFrom::Start(self.size_index)).context(KbinErrorKind::Seek)?;
      self.ser.data_buf.write_u32::<BigEndian>(size).context(KbinErrorKind::DataWrite("node size"))?;
      self.ser.data_buf.seek(SeekFrom::Start(current_pos)).context(KbinErrorKind::Seek)?;
    }

    Ok(Some(TypeHint { node_type, is_array: true, count: self.len }))
  }
}

// kbin only supports sized arrays, coerce sequence types to tuple processing
impl<'a> SerializeSeq for Tuple<'a> {
  type Ok = Option<TypeHint>;
  type Error = Error;

  fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where T: ?Sized + Serialize
  {
    <Self as SerializeTuple>::serialize_element(self, value)
  }

  fn end(self) -> Result<Self::Ok> {
    <Self as SerializeTuple>::end(self)
  }
}
