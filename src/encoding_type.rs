#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodingType {
  None,
  ASCII,
  ISO_8859_1,
  EUC_JP,
  SHIFT_JIS,
  UTF_8,
}

impl EncodingType {
  #[allow(dead_code)]
  pub fn to_byte(&self) -> u8 {
    match *self {
      EncodingType::None       => 0x00, // 0x00 >> 5 = 0
      EncodingType::ASCII      => 0x20, // 0x20 >> 5 = 1
      EncodingType::ISO_8859_1 => 0x40, // 0x40 >> 5 = 2
      EncodingType::EUC_JP     => 0x60, // 0x60 >> 5 = 3
      EncodingType::SHIFT_JIS  => 0x80, // 0x80 >> 5 = 4
      EncodingType::UTF_8      => 0xA0, // 0xA0 >> 5 = 5
    }
  }

  pub fn from_byte(byte: u8) -> Option<Self> {
    let val = match byte {
      0x00 => EncodingType::None,
      0x20 => EncodingType::ASCII,
      0x40 => EncodingType::ISO_8859_1,
      0x60 => EncodingType::EUC_JP,
      0x80 => EncodingType::SHIFT_JIS,
      0xA0 => EncodingType::UTF_8,
      _ => return None,
    };

    Some(val)
  }
}
