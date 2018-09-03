use std::fmt;
use std::net::Ipv4Addr;

use serde::de::{Deserialize, Deserializer, DeserializeSeed};
use serde_bytes::ByteBuf;

use node::Node;
use node::de::NodeSeed;
use node_types::StandardType;

mod de;
mod ser;

macro_rules! construct_types {
  (
    $(
      ($konst:ident, $($value_type:tt)*);
    )+
  ) => {
    #[derive(Clone, PartialEq)]
    pub enum Value {
      $(
        $konst($($value_type)*),
      )+
      Binary(Vec<u8>),
      Time(u32),
      Attribute(String),

      Array(StandardType, Vec<Value>),
      Node(Box<Node>),
    }

    $(
      impl From<$($value_type)*> for Value {
        fn from(value: $($value_type)*) -> Value {
          Value::$konst(value)
        }
      }
    )+

    impl Value {
      pub fn standard_type(&self) -> StandardType {
        match *self {
          $(
            Value::$konst(_) => StandardType::$konst,
          )+
          Value::Binary(_) => StandardType::Binary,
          Value::Time(_) => StandardType::Time,
          Value::Attribute(_) => StandardType::Attribute,
          Value::Array(node_type, _) => node_type,
          Value::Node(_) => StandardType::NodeStart,
        }
      }

      pub fn as_i8(&self) -> Option<i8> {
        match self {
          Value::S8(ref n) => Some(*n),
          _ => None,
        }
      }

      pub fn as_u8(&self) -> Option<u8> {
        match self {
          Value::U8(ref n) => Some(*n),
          _ => None,
        }
      }
    }

    impl<'de> DeserializeSeed<'de> for StandardType {
      type Value = Value;

      fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer<'de>
      {
        trace!("<StandardType as DeserializeSeed>::deserialize(self: {:?})", self);
        match self {
          $(
            StandardType::$konst => <$($value_type)*>::deserialize(deserializer).map(Value::$konst),
          )+
          StandardType::Binary => ByteBuf::deserialize(deserializer).map(Vec::from).map(Value::Binary),
          StandardType::Time => u32::deserialize(deserializer).map(Value::Time),
          StandardType::Attribute => String::deserialize(deserializer).map(Value::Attribute),
          StandardType::NodeStart => NodeSeed.deserialize(deserializer).map(Box::new).map(Value::Node),
          StandardType::NodeEnd |
          StandardType::FileEnd => unimplemented!(),
        }
      }
    }
  }
}

impl From<Vec<u8>> for Value {
  fn from(value: Vec<u8>) -> Value {
    Value::Binary(value)
  }
}

impl From<ByteBuf> for Value {
  fn from(value: ByteBuf) -> Value {
    Value::Binary(value.into())
  }
}

impl fmt::Debug for Value {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    macro_rules! field {
      (
        display: [$($konst_display:ident),*],
        debug_alternate: [$($konst_alternate:ident),*],
        debug: [$($konst_debug:ident),*]
      ) => {
        match *self {
          $(
            Value::$konst_display(ref v) => write!(f, concat!(stringify!($konst_display), "({})"), v),
          )*
          $(
            Value::$konst_alternate(ref v) => if f.alternate() {
              write!(f, concat!(stringify!($konst_alternate), "({:#?})"), v)
            } else {
              write!(f, concat!(stringify!($konst_alternate), "({:?})"), v)
            },
          )*
          $(
            Value::$konst_debug(ref v) => write!(f, concat!(stringify!($konst_debug), "({:?})"), v),
          )*
          Value::Binary(ref v) => write!(f, "Binary(0x{:02x?})", v),
          Value::Array(ref node_type, ref a) => if f.alternate() {
            write!(f, "Array({:?}, {:#?}", node_type, a)
          } else {
            write!(f, "Array({:?}, {:?}", node_type, a)
          },
        }
      };
    }

    field! {
      display: [
        S8, S16, S32, S64,
        U8, U16, U32, U64,
        Float, Double, Boolean
      ],
      debug_alternate: [
        Node
      ],
      debug: [
        String, Time, Ip4,
        Attribute,
        S8_2, U8_2, S16_2, U16_2, S32_2, U32_2, S64_2, U64_2, Float2, Double2, Boolean2,
        S8_3, U8_3, S16_3, U16_3, S32_3, U32_3, S64_3, U64_3, Float3, Double3, Boolean3,
        S8_4, U8_4, S16_4, U16_4, S32_4, U32_4, S64_4, U64_4, Float4, Double4, Boolean4,
        Vs16, Vu16,
        Vs8, Vu8, Vb
      ]
    }
  }
}

construct_types! {
  (S8,       i8);
  (U8,       u8);
  (S16,      i16);
  (U16,      u16);
  (S32,      i32);
  (U32,      u32);
  (S64,      i64);
  (U64,      u64);
  //(Binary,   Vec<u8>);
  (String,   String);
  (Ip4,      Ipv4Addr);
  //(Time,     u32);
  (Float,    f32);
  (Double,   f64);
  (S8_2,     [i8; 2]);
  (U8_2,     [u8; 2]);
  (S16_2,    [i16; 2]);
  (U16_2,    [u16; 2]);
  (S32_2,    [i32; 2]);
  (U32_2,    [u32; 2]);
  (S64_2,    [i64; 2]);
  (U64_2,    [u64; 2]);
  (Float2,   [f32; 2]);
  (Double2,  [f64; 2]);
  (S8_3,     [i8; 3]);
  (U8_3,     [u8; 3]);
  (S16_3,    [i16; 3]);
  (U16_3,    [u16; 3]);
  (S32_3,    [i32; 3]);
  (U32_3,    [u32; 3]);
  (S64_3,    [i64; 3]);
  (U64_3,    [u64; 3]);
  (Float3,   [f32; 3]);
  (Double3,  [f64; 3]);
  (S8_4,     [i8; 4]);
  (U8_4,     [u8; 4]);
  (S16_4,    [i16; 4]);
  (U16_4,    [u16; 4]);
  (S32_4,    [i32; 4]);
  (U32_4,    [u32; 4]);
  (S64_4,    [i64; 4]);
  (U64_4,    [u64; 4]);
  (Float4,   [f32; 4]);
  (Double4,  [f64; 4]);
  //(Attribute, String);
  // no 47
  (Vs8,      [i8; 16]);
  (Vu8,      [u8; 16]);
  (Vs16,     [i16; 8]);
  (Vu16,     [u16; 8]);
  (Boolean,  bool);
  (Boolean2, [bool; 2]);
  (Boolean3, [bool; 3]);
  (Boolean4, [bool; 4]);
  (Vb,       [bool; 16]);
}
