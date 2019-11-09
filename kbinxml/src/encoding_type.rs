use std::fmt;
use std::string::FromUtf8Error;

/// The `encoding_rs` crate uses the following to describe their counterparts:
///
/// `SHIFT_JIS`    => `WINDOWS_31J`
/// `WINDOWS_1252` => `ISO-8859-1`
use encoding_rs::{Encoding, EUC_JP, SHIFT_JIS, UTF_8, WINDOWS_1252};
use snafu::{ResultExt, Snafu};

#[derive(Debug, Snafu)]
pub enum EncodingError {
    #[snafu(display("Unknown encoding"))]
    UnknownEncoding,

    #[snafu(display("Another encoding was used to decode the input: {:?}", actual))]
    MismatchedDecode { actual: &'static Encoding },

    #[snafu(display("Another encoding was used to encode the output: {:?}", actual))]
    MismatchedEncode { actual: &'static Encoding },

    #[snafu(display("Unmappable characters found in input"))]
    UnmappableCharacters,

    #[snafu(display("Invalid ASCII character at index: {}", index))]
    InvalidAscii { index: usize },

    #[snafu(display("Failed to interpret string as UTF-8"))]
    InvalidUtf8 { source: FromUtf8Error },

    #[snafu(display("Failed to convert string to alternate encoding"))]
    Convert,
}

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
    pub fn from_byte(byte: u8) -> Result<Self, EncodingError> {
        let val = match byte {
            0x00 => EncodingType::None,
            0x20 => EncodingType::ASCII,
            0x40 => EncodingType::ISO_8859_1,
            0x60 => EncodingType::EUC_JP,
            0x80 => EncodingType::SHIFT_JIS,
            0xA0 => EncodingType::UTF_8,
            _ => return Err(EncodingError::UnknownEncoding),
        };

        Ok(val)
    }

    pub fn from_encoding(encoding: &'static Encoding) -> Result<Self, EncodingError> {
        match encoding {
            e if e == WINDOWS_1252 => Ok(EncodingType::ISO_8859_1),
            e if e == EUC_JP => Ok(EncodingType::EUC_JP),
            e if e == SHIFT_JIS => Ok(EncodingType::SHIFT_JIS),
            e if e == UTF_8 => Ok(EncodingType::UTF_8),
            _ => return Err(EncodingError::UnknownEncoding),
        }
    }

    pub fn from_label(label: &[u8]) -> Result<Self, EncodingError> {
        Encoding::for_label(label)
            .ok_or(EncodingError::UnknownEncoding)
            .and_then(Self::from_encoding)
    }

    pub fn to_byte(&self) -> u8 {
        match *self {
            EncodingType::None => 0x00,       // 0x00 >> 5 = 0
            EncodingType::ASCII => 0x20,      // 0x20 >> 5 = 1
            EncodingType::ISO_8859_1 => 0x40, // 0x40 >> 5 = 2
            EncodingType::EUC_JP => 0x60,     // 0x60 >> 5 = 3
            EncodingType::SHIFT_JIS => 0x80,  // 0x80 >> 5 = 4
            EncodingType::UTF_8 => 0xA0,      // 0xA0 >> 5 = 5
        }
    }

    pub fn name(&self) -> Option<&'static str> {
        match *self {
            EncodingType::None => None,
            EncodingType::ASCII => None,
            EncodingType::ISO_8859_1 => Some(WINDOWS_1252.name()),
            EncodingType::EUC_JP => Some(EUC_JP.name()),
            EncodingType::SHIFT_JIS => Some(SHIFT_JIS.name()),
            EncodingType::UTF_8 => Some(UTF_8.name()),
        }
    }

    fn decode_ascii(input: &[u8]) -> Result<String, EncodingError> {
        // ASCII only goes up to 0x7F
        match input.iter().position(|&ch| ch >= 0x80) {
            Some(index) => Err(EncodingError::InvalidAscii { index }),
            None => String::from_utf8(input.to_vec()).context(InvalidUtf8),
        }
    }

    fn encode_ascii(input: &str) -> Result<Vec<u8>, EncodingError> {
        // ASCII only goes up to 0x7F
        match input.as_bytes().iter().position(|&ch| ch >= 0x80) {
            Some(index) => Err(EncodingError::InvalidAscii { index }),
            None => Ok(input.as_bytes().to_vec()),
        }
    }

    fn decode_with_encoding(
        encoding: &'static Encoding,
        input: &[u8],
    ) -> Result<String, EncodingError> {
        let (output, actual, character_replaced) = encoding.decode(input);

        //eprintln!("character replaced: {}", character_replaced);

        if character_replaced {
            warn!("Character replacement occured with: {:?}", output);
        }

        // `EncodingType::SHIFT_JIS` will ignore invalid characters because Konami's
        // implementation will include invalid characters.
        if encoding != actual {
            Err(EncodingError::MismatchedDecode { actual })
        } else if !character_replaced || encoding == SHIFT_JIS {
            Ok(output.into_owned())
        } else {
            Err(EncodingError::UnmappableCharacters)
        }
    }

    fn encode_with_encoding(
        encoding: &'static Encoding,
        input: &str,
    ) -> Result<Vec<u8>, EncodingError> {
        let (output, actual, had_unmappable_characters) = encoding.encode(input);

        if encoding != actual {
            Err(EncodingError::MismatchedEncode { actual })
        } else if had_unmappable_characters {
            Err(EncodingError::UnmappableCharacters)
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
    pub fn decode_bytes(&self, input: &[u8]) -> Result<String, EncodingError> {
        match *self {
            EncodingType::None | EncodingType::UTF_8 => {
                String::from_utf8(input.to_vec()).context(InvalidUtf8)
            },

            EncodingType::ASCII => Self::decode_ascii(input),
            EncodingType::ISO_8859_1 => Self::decode_with_encoding(WINDOWS_1252, input),
            EncodingType::EUC_JP => Self::decode_with_encoding(EUC_JP, input),
            EncodingType::SHIFT_JIS => Self::decode_with_encoding(SHIFT_JIS, input),
        }
    }

    /// Encode bytes using the encoding definition from the `encoding` crate.
    ///
    /// A `Some` value indicates the encoding should be used from the `encoding`
    /// crate. A `None` value indicates Rust's own UTF-8 handling should be used.
    pub fn encode_bytes(&self, input: &str) -> Result<Vec<u8>, EncodingError> {
        let mut result = match *self {
            EncodingType::None | EncodingType::UTF_8 => input.as_bytes().to_vec(),

            EncodingType::ASCII => Self::encode_ascii(input)?,
            EncodingType::ISO_8859_1 => Self::encode_with_encoding(WINDOWS_1252, input)?,
            EncodingType::EUC_JP => Self::encode_with_encoding(EUC_JP, input)?,
            EncodingType::SHIFT_JIS => Self::encode_with_encoding(SHIFT_JIS, input)?,
        };

        // Add trailing null byte
        result.reserve_exact(1);
        result.push(0);

        Ok(result)
    }
}
