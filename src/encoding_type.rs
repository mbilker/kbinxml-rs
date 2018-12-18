use std::fmt;

use crate::error::{KbinError, KbinErrorKind};
use failure::ResultExt;

/// The `encoding_rs` crate uses the following to describe their counterparts:
///
/// `SHIFT_JIS`    => `WINDOWS_31J`
/// `WINDOWS_1252` => `ISO-8859-1`
use encoding_rs::{Encoding, EUC_JP, SHIFT_JIS, UTF_8, WINDOWS_1252};

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

impl Default for EncodingType {
  fn default() -> Self {
    EncodingType::SHIFT_JIS
  }
}

impl fmt::Display for EncodingType {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let encoding = match *self {
      EncodingType::None => "None",
      EncodingType::ASCII => "ASCII",
      EncodingType::ISO_8859_1 => "ISO-8859-1",
      EncodingType::EUC_JP => "EUC-JP",
      EncodingType::SHIFT_JIS => "SHIFT-JIS",
      EncodingType::UTF_8 => "UTF-8",
    };

    write!(f, "{}", encoding)
  }
}

impl EncodingType {
  pub fn from_byte(byte: u8) -> Result<Self, KbinError> {
    let val = match byte {
      0x00 => EncodingType::None,
      0x20 => EncodingType::ASCII,
      0x40 => EncodingType::ISO_8859_1,
      0x60 => EncodingType::EUC_JP,
      0x80 => EncodingType::SHIFT_JIS,
      0xA0 => EncodingType::UTF_8,
      _ => return Err(KbinErrorKind::UnknownEncoding.into()),
    };

    Ok(val)
  }

  pub fn from_encoding(encoding: &'static Encoding) -> Result<Self, KbinError> {
    let val = match encoding {
      e if e == WINDOWS_1252 => EncodingType::ISO_8859_1,
      e if e == EUC_JP       => EncodingType::EUC_JP,
      e if e == SHIFT_JIS    => EncodingType::SHIFT_JIS,
      e if e == UTF_8        => EncodingType::UTF_8,
      _ => return Err(KbinErrorKind::UnknownEncoding.into()),
    };

    Ok(val)
  }

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

  pub fn name(&self) -> Option<&'static str> {
    match *self {
      EncodingType::None       => None,
      EncodingType::ASCII      => None,
      EncodingType::ISO_8859_1 => Some(WINDOWS_1252.name()),
      EncodingType::EUC_JP     => Some(EUC_JP.name()),
      EncodingType::SHIFT_JIS  => Some(SHIFT_JIS.name()),
      EncodingType::UTF_8      => Some(UTF_8.name()),
    }
  }

  fn decode_ascii(input: &[u8]) -> Result<String, KbinError> {
    // ASCII only goes up to 0x7F
    match input.iter().position(|&ch| ch >= 0x80) {
      Some(first_error) => {
        Err(format_err!("Invalid ASCII character at index: {}", first_error)
          .context(KbinErrorKind::Encoding)
          .into())
      },
      None => {
        let output = String::from_utf8(input.to_vec()).context(KbinErrorKind::Utf8)?;

        Ok(output)
      },
    }
  }

  fn encode_ascii(input: &str) -> Result<Vec<u8>, KbinError> {
    // ASCII only goes up to 0x7F
    match input.as_bytes().iter().position(|&ch| ch >= 0x80) {
      Some(first_error) => {
        Err(format_err!("Unrepresentable character found at index: {}", first_error)
          .context(KbinErrorKind::Encoding)
          .into())
      },
      None => {
        Ok(input.as_bytes().to_vec())
      },
    }
  }

  fn decode_with_encoding(encoding: &'static Encoding, input: &[u8]) -> Result<String, KbinError> {
    let (output, actual_encoding, character_replaced) = encoding.decode(input);

    //eprintln!("character replaced: {}", character_replaced);

    if character_replaced {
      warn!("Character replacement occured with: {:?}", output);
    }

    // `EncodingType::SHIFT_JIS` will ignore invalid characters because Konami's
    // implementation will include invalid characters.
    if encoding != actual_encoding {
      Err(format_err!("Another encoding was used to decode the output: {:?}", actual_encoding)
        .context(KbinErrorKind::Encoding)
        .into())
    } else if !character_replaced || encoding == SHIFT_JIS {
      Ok(output.into_owned())
    } else {
      Err(KbinErrorKind::Encoding.into())
    }
  }

  fn encode_with_encoding(encoding: &'static Encoding, input: &str) -> Result<Vec<u8>, KbinError> {
    let (output, actual_encoding, had_unmappable_characters) = encoding.encode(input);

    if encoding != actual_encoding {
      Err(format_err!("Another encoding was used to encode the output: {:?}", actual_encoding)
        .context(KbinErrorKind::Encoding)
        .into())
    } else if had_unmappable_characters {
      Err(format_err!("had unmappable characters").context(KbinErrorKind::Encoding).into())
    } else {
      Ok(output.into_owned())
    }
  }

  /// Decode bytes using the encoding definition from the `encoding` crate.
  ///
  /// A `Some` value indicates an encoding should be used from the `encoding`
  /// crate. A `None` value indicates Rust's own UTF-8 handling should be used.
  ///
  /// `EncodingType::SHIFT_JIS` will ignore invalid characters because Konami's
  /// implementation will include invalid characters.
  pub fn decode_bytes(&self, input: &[u8]) -> Result<String, KbinError> {
    match *self {
      EncodingType::None |
      EncodingType::UTF_8 => String::from_utf8(input.to_vec()).map_err(KbinError::from),

      EncodingType::ASCII      => Self::decode_ascii(input),
      EncodingType::ISO_8859_1 => Self::decode_with_encoding(WINDOWS_1252, input),
      EncodingType::EUC_JP     => Self::decode_with_encoding(EUC_JP, input),
      EncodingType::SHIFT_JIS  => Self::decode_with_encoding(SHIFT_JIS, input),
    }
  }

  /// Encode bytes using the encoding definition from the `encoding` crate.
  ///
  /// A `Some` value indicates the encoding should be used from the `encoding`
  /// crate. A `None` value indicates Rust's own UTF-8 handling should be used.
  pub fn encode_bytes(&self, input: &str) -> Result<Vec<u8>, KbinError> {
    let mut result = match *self {
      EncodingType::None |
      EncodingType::UTF_8 => input.as_bytes().to_vec(),

      EncodingType::ASCII      => Self::encode_ascii(input)?,
      EncodingType::ISO_8859_1 => Self::encode_with_encoding(WINDOWS_1252, input)?,
      EncodingType::EUC_JP     => Self::encode_with_encoding(EUC_JP, input)?,
      EncodingType::SHIFT_JIS  => Self::encode_with_encoding(SHIFT_JIS, input)?,
    };

    // Add trailing null byte
    result.reserve_exact(1);
    result.push(0);

    Ok(result)
  }
}
