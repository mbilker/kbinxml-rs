use byteorder::WriteBytesExt;
use failure::ResultExt;
use serde::ser::{Serialize, SerializeStruct};

use node_types::StandardType;
use error::KbinErrorKind;
use ser::{Error, Result, Serializer, TypeHint, ARRAY_MASK};
use sixbit::pack_sixbit;

pub struct Struct<'a> {
  ser: &'a mut Serializer,
  name: &'static str,
}

impl<'a> Struct<'a> {
  pub fn new(ser: &'a mut Serializer, name: &'static str, len: usize) -> Result<Self> {
    debug!("Struct::new(name: {}, len: {}) => hierarchy: {:?}", name, len, ser.hierarchy);

    // Restrict bounds of immutable borrow from `hierarchy` Vec
    {
      // The key name would have been pushed to the stack in `serialize_field`
      // before calling `serialize` on the value
      let name = if let Some(key) = ser.hierarchy.last() {
        trace!("Struct::new(name: {}) => found key name: {}", name, key);
        key
      } else {
        name
      };

      let node_type = StandardType::NodeStart;
      ser.node_buf.write_u8(node_type.id).context(KbinErrorKind::DataWrite(node_type.name))?;
      pack_sixbit(&mut *ser.node_buf, name)?;
    }

    // The `Vec` cannot be borrowed as mutable in an `else` condition because
    // of the immutable borrow made in the previous if statement, so this is a
    // workaround
    ser.hierarchy.push(name);
    trace!("Struct::new(name: {}) => hierarchy: {:?}", name, ser.hierarchy);

    Ok(Self {
      ser,
      name,
    })
  }
}

impl<'a> SerializeStruct for Struct<'a> {
  type Ok = Option<TypeHint>;
  type Error = Error;

  fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where T: ?Sized + Serialize
  {
    // Push key name onto stack so if the value is a struct, it will pick up
    // the correct name
    self.ser.hierarchy.push(key);
    debug!("SerializeStruct(name: {})::serialize_field(key: {}) => hierarchy: {:?}", self.name, key, self.ser.hierarchy);

    // Serialize methods that return `None` will not be written
    if let Some(hint) = value.serialize(&mut *self.ser)? {
      let node_type = hint.node_type;
      let array_mask = if hint.is_array { ARRAY_MASK } else { 0 };
      debug!("SerializeStruct(name: {})::serialize_field(key: {}) => hint: {:?}", self.name, key, hint);

      // Struct handler outputs the `NodeStart` event by itself. Avoid repeating it.
      if node_type != StandardType::NodeStart {
        self.ser.node_buf.write_u8(node_type.id | array_mask).context(KbinErrorKind::DataWrite(node_type.name))?;
        pack_sixbit(&mut *self.ser.node_buf, key)?;

        // TODO: Make sure this does not prematurely end nodes
        if node_type != StandardType::Attribute {
          self.ser.node_buf.write_u8(StandardType::NodeEnd.id | ARRAY_MASK).context(KbinErrorKind::DataWrite("node end"))?;
        }
      }
    }

    // Pop the name off the stack that was added earlier
    let val = self.ser.hierarchy.pop();
    debug!("SerializeStruct(name: {})::serialize_field() => popped: {:?}", self.name, val);

    Ok(())
  }

  fn end(self) -> Result<Self::Ok> {
    debug!("SerializeStruct(name: {})::end()", self.name);
    self.ser.node_buf.write_u8(StandardType::NodeEnd.id | ARRAY_MASK).context(KbinErrorKind::DataWrite("node end"))?;

    let val = self.ser.hierarchy.pop();
    trace!("SerializeStruct(name: {})::end() => popped: {:?}, hierarchy: {:?}, node_buf: {:02x?}", self.name, val, self.ser.hierarchy, self.ser.node_buf.get_ref());

    Ok(Some(TypeHint::from_type(StandardType::NodeStart)))
  }
}
