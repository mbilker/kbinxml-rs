use crate::compression::Compression;
use crate::encoding_type::EncodingType;

#[derive(Clone, Debug, Default)]
pub struct Options {
  pub(crate) compression: Compression,
  pub(crate) encoding: EncodingType,
}

impl Options {
  pub fn new(
    compression: Compression,
    encoding: EncodingType,
  ) -> Self {
    Self {
      compression,
      encoding,
    }
  }

  pub fn with_encoding(encoding: EncodingType) -> Self {
    Self {
      encoding,
      ..Default::default()
    }
  }
}
