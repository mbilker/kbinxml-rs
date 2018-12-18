use std::fmt;
use std::marker::PhantomData;

use serde::de::{Deserialize, Deserializer, EnumAccess, Error, IntoDeserializer, SeqAccess, VariantAccess, Visitor};
use serde::de::value::SeqDeserializer;

use crate::node::Marshal;
use crate::node_types::StandardType;
use crate::value::Value;

impl<'de> Deserialize<'de> for Value {
  #[inline]
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: Deserializer<'de>
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
        where E: Error
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
        where E: Error
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
        where E: Error
      {
        trace!("ValueVisitor::visit_bytes(value: 0x{:02x?})", value);
        self.visit_byte_buf(value.to_vec())
      }

      #[inline]
      fn visit_borrowed_bytes<E>(self, value: &'de [u8]) -> Result<Self::Value, E>
        where E: Error
      {
        trace!("ValueVisitor::visit_borrowed_bytes(value: 0x{:02x?})", value);
        self.visit_byte_buf(value.to_vec())
      }

      #[inline]
      fn visit_byte_buf<E>(self, value: Vec<u8>) -> Result<Self::Value, E> {
        trace!("ValueVisitor::visit_byte_buf(value: 0x{:02x?})", value);
        Ok(Value::Binary(value))
      }

      #[inline]
      fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer<'de>
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

        while let Some(elem) = seq.next_element()? {
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
      fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer<'de>
      {
        trace!("ValueVisitor::visit_newtype_struct()");

        let marshal: Marshal = Marshal::deserialize(deserializer)?;
        debug!("ValueVisitor::visit_newtype_struct() => marshal: {:?}", marshal);

        marshal.into_inner().as_value().ok_or_else(|| D::Error::custom("`Marshal` must contain `Value` not `Node`"))
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
        visit_f64 f64 Double
      }
    }

    deserializer.deserialize_any(ValueVisitor)
  }
}

pub struct ValueDeserializer<E> {
  value: Value,
  marker: PhantomData<E>,
}

impl<'de, E> Deserializer<'de> for ValueDeserializer<E>
  where E: Error
{
  type Error = E;

  /// Trigger `Ipv4Addr`'s deserializer to use octets rather than a string for
  /// deserialization
  fn is_human_readable(&self) -> bool {
    false
  }

  #[inline]
  fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    trace!("ValueDeserializer::deserialize_any(value: {:?})", self.value);

    macro_rules! tuple {
      ($($type:ident),*) => {
        match self.value {
          Value::S8(n) => visitor.visit_i8(n),
          Value::U8(n) => visitor.visit_u8(n),
          Value::S16(n) => visitor.visit_i16(n),
          Value::U16(n) => visitor.visit_u16(n),
          Value::S32(n) => visitor.visit_i32(n),
          Value::U32(n) => visitor.visit_u32(n),
          Value::S64(n) => visitor.visit_i64(n),
          Value::U64(n) => visitor.visit_u64(n),
          Value::Binary(buf) => visitor.visit_byte_buf(buf),
          Value::String(s) => visitor.visit_string(s),
          Value::Ip4(n) => SeqDeserializer::new(n.octets().into_iter().cloned()).deserialize_any(visitor),
          Value::Float(n) => visitor.visit_f32(n),
          Value::Double(n) => visitor.visit_f64(n),
          Value::Boolean(n) => visitor.visit_bool(n),

          $(
            Value::$type(n) => SeqDeserializer::new(n.into_iter().cloned()).deserialize_any(visitor),
          )*

          Value::Time(n) => visitor.visit_u32(n),
          Value::Attribute(s) => visitor.visit_string(s),

          Value::Array(_, v) => SeqDeserializer::new(v.into_iter()).deserialize_any(visitor),
          Value::Node(node) => node.into_deserializer().deserialize_any(visitor),
        }
      };
    }

    tuple! {
      S8_2, U8_2, S16_2, U16_2, S32_2, U32_2, S64_2, U64_2, Float2, Double2, Boolean2,
      S8_3, U8_3, S16_3, U16_3, S32_3, U32_3, S64_3, U64_3, Float3, Double3, Boolean3,
      S8_4, U8_4, S16_4, U16_4, S32_4, U32_4, S64_4, U64_4, Float4, Double4, Boolean4,
      Vs16, Vu16,
      Vs8, Vu8, Vb
    }
  }

  forward_to_deserialize_any! {
    bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str
    string bytes byte_buf option unit unit_struct newtype_struct seq
    tuple tuple_struct map struct enum identifier ignored_any
  }
}

impl<'de, E> IntoDeserializer<'de, E> for Value
  where E: Error
{
  type Deserializer = ValueDeserializer<E>;

  fn into_deserializer(self) -> Self::Deserializer {
    ValueDeserializer {
      value: self,
      marker: PhantomData,
    }
  }
}
