use serde::ser::{Serialize, SerializeTuple};

use error::KbinErrorKind;
use node_types::StandardType;
use ser::{Error, Result, Serializer, TypeHint};
use ser::buffer::BufferSerializer;

/// Tuple handler for serialization.
///
/// Kbin tuple types are monotype (all tuple elements have the same type)
/// which differs from Rust's tuples that can have different types for each
/// element.
///
/// This key difference is what makes it harder to seralize tuples, which is
/// why the `BufferSerializer` is used to serialize tuple elements to an
/// intermediate byte array before running the write logic. Kbin's write logic
/// depends on the size of the type, which is taken care of by
/// `ByteBuffer::write_aligned`.
pub struct Tuple<'a> {
  ser: &'a mut Serializer,
  buffer: BufferSerializer,

  node_type: Option<StandardType>,
  len: usize,
}

impl<'a> Tuple<'a> {
  pub fn new(ser: &'a mut Serializer, len: usize) -> Self {
    trace!("Tuple::new(len: {})", len);

    Self {
      ser,
      buffer: BufferSerializer::new(),
      node_type: None,
      len,
    }
  }

  fn find_standard_type(&self) -> Result<StandardType> {
    let base = self.node_type.ok_or(KbinErrorKind::MissingBaseType)?;
    let combined = StandardType::find_type(base, self.len);
    debug!("find_standard_type => StandardType::find_type(base: {:?}, len: {}) = {:?}", base, self.len, combined);

    Ok(combined)
  }
}

impl<'a> SerializeTuple for Tuple<'a> {
  type Ok = Option<TypeHint>;
  type Error = Error;

  fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where T: ?Sized + Serialize
  {
    let node_type = value.serialize(&mut self.buffer)?;

    // Rust tuple types can have different types per element, this is not
    // permitted by kbin
    if let Some(known) = self.node_type {
      if known != node_type {
        return Err(KbinErrorKind::TypeMismatch(*known, *node_type).into());
      }
    } else {
      self.node_type = Some(node_type);
    }

    Ok(())
  }

  fn end(self) -> Result<Self::Ok> {
    let node_type = self.find_standard_type()?;
    let buffer = self.buffer.into_inner();
    debug!("<Tuple as SerializeTuple>::end() => buffer: {:?}, node_type: {:?}", buffer, node_type);

    self.ser.data_buf.write_aligned(*node_type, &buffer)?;

    Ok(Some(TypeHint::from_type(node_type)))
  }
}
