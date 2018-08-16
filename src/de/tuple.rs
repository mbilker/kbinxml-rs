use std::fmt;
use std::io::Cursor;

use byteorder::{BigEndian, ReadBytesExt};
use failure::ResultExt;
use serde::de::{self, Deserializer, DeserializeSeed, Expected, IntoDeserializer, SeqAccess, Visitor};

use error::{Error, KbinErrorKind};
use node_types::StandardType;

pub struct TupleBytesDeserializer<'input> {
  node_type: StandardType,
  input: Cursor<&'input [u8]>,
}

impl<'input> TupleBytesDeserializer<'input> {
  pub fn new(node_type: StandardType, input: &'input [u8]) -> Self {
    Self {
      node_type,
      input: Cursor::new(input),
    }
  }

  fn end(self) -> Result<(), Error> {
    let position = self.input.position() as usize;
    let len = self.input.get_ref().len();

    // The position will be equal to len if every element was read
    if position == len {
      Ok(())
    } else {
      Err(de::Error::invalid_length(position, &ExpectedInSeq(len)))
    }
  }
}

struct ExpectedInSeq(usize);

impl Expected for ExpectedInSeq {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    if self.0 == 1 {
      write!(formatter, "1 element in input")
    } else {
      write!(formatter, "{} elements in input", self.0)
    }
  }
}

impl<'de, 'input> Deserializer<'de> for TupleBytesDeserializer<'input> {
  type Error = Error;

  fn deserialize_any<V>(mut self, visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de>
  {
    let value = visitor.visit_seq(&mut self)?;
    self.end()?;
    Ok(value)
  }

  forward_to_deserialize_any! {
    bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
    bytes byte_buf option unit unit_struct newtype_struct seq tuple
    tuple_struct map struct enum identifier ignored_any
  }
}

impl<'de, 'input> SeqAccess<'de> for TupleBytesDeserializer<'input> {
  type Error = Error;

  fn next_element_seed<V>(&mut self, seed: V) -> Result<Option<V::Value>, Self::Error>
    where V: DeserializeSeed<'de>
  {
    macro_rules! read_primitive {
      (
        byte: [
          $($byte_read_method:ident $byte_base_konst:ident [$($byte_konst:ident)*]),*
        ],
        bool: [
          $($bool_base_konst:ident [$($bool_konst:ident)*]),*
        ],
        $($read_method:ident $size:tt $base_konst:ident [$($konst:ident)*]),*
      ) => {
        match self.node_type {
          $(
            $(
            StandardType::$byte_konst |
            )*
            StandardType::$byte_base_konst => {
              let value = self.input.$byte_read_method().context(KbinErrorKind::DataRead(1))?;
              trace!("<TupleBytesDeserializer as SeqAccess>::next_element_seed(node_type: {:?}, byte) => value: {:?}", self.node_type, value);

              seed.deserialize(value.into_deserializer()).map(Some)
            },
          )*
          $(
            $(
            StandardType::$bool_konst |
            )*
            StandardType::$bool_base_konst => {
              let value = match self.input.read_u8().context(KbinErrorKind::DataRead(1))? {
                0x00 => false,
                0x01 => true,
                value => return Err(Error::Message(format!("invalid value for boolean: {0:?} (0x{0:x})", value))),
              };
              trace!("<TupleBytesDeserializer as SeqAccess>::next_element_seed(node_type: {:?}, bool) => value: {:?}", self.node_type, value);

              seed.deserialize(value.into_deserializer()).map(Some)
            },
          )*
          $(
            $(
            StandardType::$konst |
            )*
            StandardType::$base_konst => {
              let value = self.input.$read_method::<BigEndian>().context(KbinErrorKind::DataRead($size))?;
              trace!("<TupleBytesDeserializer as SeqAccess>::next_element_seed(node_type: {:?}) => value: {:?}", self.node_type, value);

              seed.deserialize(value.into_deserializer()).map(Some)
            },
          )*
          StandardType::Binary |
          StandardType::String |
          StandardType::Time |
          StandardType::Attribute |
          StandardType::NodeStart |
          StandardType::NodeEnd |
          StandardType::FileEnd => unimplemented!(),
        }
      };
    }

    let value = read_primitive! {
      byte: [
        read_u8 U8 [ U8_2 U8_3 U8_4 Vu8 Ip4 ],
        read_i8 S8 [ S8_2 S8_3 S8_4 Vs8 ]
      ],
      bool: [
        Boolean [ Boolean2 Boolean3 Boolean4 Vb ]
      ],
      read_u16 2 U16 [ U16_2 U16_3 U16_4 Vu16 ],
      read_i16 2 S16 [ S16_2 S16_3 S16_4 Vs16 ],
      read_u32 4 U32 [ U32_2 U32_3 U32_4 ],
      read_i32 4 S32 [ S32_2 S32_3 S32_4 ],
      read_u64 8 U64 [ U64_2 U64_3 U64_4 ],
      read_i64 8 S64 [ S64_2 S64_3 S64_4 ],
      read_f32 4 Float [ Float2 Float3 Float4 ],
      read_f64 8 Double [ Double2 Double3 Double4 ]
    };

    value
  }
}
