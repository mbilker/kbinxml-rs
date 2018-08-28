use std::fmt;

use serde::de::{self, Deserialize, EnumAccess, Error, SeqAccess, VariantAccess, Visitor};

use node_types::StandardType;
use value::Value;

impl<'de> Deserialize<'de> for Value {
  #[inline]
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: de::Deserializer<'de>
  {
    trace!("<Value as Deserialize>::deserialize()");

    struct ValueVisitor;

    macro_rules! visit_rule {
      ($($method:ident $type:tt $konst:ident),*) => {
        $(
          #[inline]
          fn $method<E>(self, value: $type) -> Result<Self::Value, E> {
            trace!("ValueVisitor::{}(value: {:?})", stringify!($method), value);
            Ok(Value::$konst(value))
          }
        )*
      };
    }

    impl<'de> Visitor<'de> for ValueVisitor {
      type Value = Value;

      fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("any valid kbin value (for Value)")
      }

      #[inline]
      fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where E: de::Error
      {
        trace!("ValueVisitor::visit_string(value: {:?})", value);

        if value.starts_with("attr_") {
          Ok(Value::Attribute(String::from(&value["attr_".len()..])))
        } else {
          Ok(Value::String(value))
        }
      }

      #[inline]
      fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where E: de::Error
      {
        trace!("ValueVisitor::visit_str(value: {:?})", value);

        if value.starts_with("attr_") {
          Ok(Value::Attribute(String::from(&value["attr_".len()..])))
        } else {
          Ok(Value::String(String::from(value)))
        }
      }

      #[inline]
      fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
        where E: de::Error
      {
        trace!("ValueVisitor::visit_bytes(value: 0x{:02x?})", value);
        self.visit_byte_buf(value.to_vec())
      }

      #[inline]
      fn visit_byte_buf<E>(self, value: Vec<u8>) -> Result<Self::Value, E> {
        trace!("ValueVisitor::visit_byte_buf(value: 0x{:02x?})", value);
        Ok(Value::Binary(value))
      }

      #[inline]
      fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: de::Deserializer<'de>
      {
        trace!("ValueVisitor::visit_some()");
        Deserialize::deserialize(deserializer)
      }

      #[inline]
      fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where A: SeqAccess<'de>
      {
        trace!("ValueVisitor::visit_seq()");

        let mut vec = Vec::new();
        let mut array_node_type = None;

        while let Some(elem) = try!(seq.next_element()) {
          let elem: Value = elem;
          let node_type = elem.standard_type();

          // Ensure that all elements in the `Vec` are of the same `Value` variant
          if let Some(array_node_type) = array_node_type {
            if array_node_type != node_type {
              return Err(A::Error::custom("All values in `Value::Array` must be the same node type"));
            }
          } else {
            array_node_type = Some(node_type);
          }

          debug!("ValueVisitor::visit_seq() => node_type: {:?}, elem: {:?}", node_type, elem);
          vec.push(elem);
        }

        let array_node_type = array_node_type.ok_or_else(|| A::Error::custom("`Value::Array` must have node type"))?;
        Ok(Value::Array(array_node_type, vec))
      }

      #[inline]
      fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
        where A: EnumAccess<'de>
      {
        trace!("ValueVisitor::visit_enum()");
        let (id, access): (u8, _) = data.variant()?;
        let node_type = StandardType::from_u8(id);
        debug!("ValueVisitor::visit_enum() => id: {}, node_type: {:?}", id, node_type);
        let value = access.newtype_variant_seed(node_type)?;
        debug!("ValueVisitor::visit_enum() => value: {:?}", value);
        Ok(value)
      }

      visit_rule! {
        visit_bool bool Boolean,
        visit_i8  i8  S8,
        visit_i16 i16 S16,
        visit_i32 i32 S32,
        visit_i64 i64 S64,
        visit_u8  u8  U8,
        visit_u16 u16 U16,
        visit_u32 u32 U32,
        visit_u64 u64 U64,
        visit_f32 f32 Float,
        visit_f64 f64 Double,
        visit_string String String
      }
    }

    deserializer.deserialize_any(ValueVisitor)
  }
}
