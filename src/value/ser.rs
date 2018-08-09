use serde::ser::{Serialize, SerializeTupleStruct};

use value::Value;

impl Serialize for Value {
  #[inline]
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: ::serde::Serializer
  {
    trace!("<Value as Serialize>::serialize()");

    macro_rules! tuple {
      ($($type:ident),*) => {
        match *self {
          Value::S8(ref n) => n.serialize(serializer),
          Value::U8(ref n) => n.serialize(serializer),
          Value::S16(ref n) => n.serialize(serializer),
          Value::U16(ref n) => n.serialize(serializer),
          Value::S32(ref n) => n.serialize(serializer),
          Value::U32(ref n) => n.serialize(serializer),
          Value::S64(ref n) => n.serialize(serializer),
          Value::U64(ref n) => n.serialize(serializer),
          Value::Binary(ref buf) => buf.serialize(serializer),
          Value::String(ref s) => serializer.serialize_str(s),
          Value::Ip4(ref v) => {
            let mut custom = serializer.serialize_tuple_struct("ip4", 4)?;
            custom.serialize_field(v)?;
            custom.end()
          },
          Value::Float(f) => serializer.serialize_f32(f),
          Value::Double(d) => serializer.serialize_f64(d),
          Value::Boolean(b) => serializer.serialize_bool(b),

          $(
            Value::$type(ref v) => v.serialize(serializer),
          )*
          Value::Time(ref n) => {
            let mut custom = serializer.serialize_tuple_struct("time", 4)?;
            custom.serialize_field(n)?;
            custom.end()
          },
          Value::Attribute(ref s) => serializer.serialize_str(&format!("attr_{}", s)),

          Value::Array(ref a) => a.serialize(serializer),
          Value::Map(ref m) => m.serialize(serializer),
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
}
