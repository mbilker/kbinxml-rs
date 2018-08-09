use std::fmt;

use indexmap::IndexMap;
use serde;
use serde::de::{self, Deserialize, EnumAccess, Error, MapAccess, SeqAccess, VariantAccess, Visitor};

use node_types::StandardType;
use value::Value;

impl<'de> Deserialize<'de> for Value {
  #[inline]
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de>
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
        formatter.write_str("any valid kbin value")
      }

      #[inline]
      fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where E: de::Error
      {
        trace!("ValueVisitor::visit_str(value: {:?})", value);
        self.visit_string(String::from(value))
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
        where D: serde::Deserializer<'de>
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

        while let Some(elem) = try!(seq.next_element()) {
          debug!("ValueVisitor::visit_seq() => elem: {:?}", elem);
          vec.push(elem);
        }

        Ok(Value::Array(vec))
      }

      #[inline]
      fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where A: MapAccess<'de>
      {
        trace!("ValueVisitor::visit_map()");

        let mut values: IndexMap<String, Self::Value> = IndexMap::new();

        while let Some((key, value)) = try!(map.next_entry()) {
          // Check to see if this is an attribute
          let key: String = key;
          let (key, value) = if key.starts_with("attr_") {
            let inner = match value {
              Value::String(s) => s,
              _ => return Err(A::Error::custom("Key that starts with 'attr_' must be a string")),
            };

            (&key["attr_".len()..], Value::Attribute(inner))
          } else {
            (key.as_str(), value)
          };

          debug!("ValueVisitor::visit_map() => key: {:?}, value: {:?}", key, value);
          if values.contains_key(key) {
            let replace = match values.get_mut(key).expect("Key must exist from `contains_key`") {
              Value::Array(ref mut arr) => {
                arr.push(value);
                false
              },
              // Replace the `Value` with an array of `Value`s
              _ => true,
            };

            if replace {
              let entry = values.remove(key).expect("Key must exist from `contains_key`");
              values.insert(key.to_owned(), Value::Array(vec![entry]));
            }
          } else {
            values.insert(key.to_owned(), value);
          }
        }

        Ok(Value::Map(values))
      }

      #[inline]
      fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
        where A: EnumAccess<'de>
      {
        trace!("ValueVisitor::visit_enum()");
        let (variant, access): (Value, _) = data.variant()?;
        debug!("ValueVisitor::visit_enum() => variant: {:?}", variant);
        let name = match variant {
          Value::String(s) => s,
          _ => return Err(A::Error::custom("Enum variant must be a string")),
        };
        let node_type = StandardType::from_name(&name);
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
