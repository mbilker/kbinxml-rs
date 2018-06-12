use encoding_type::EncodingType;

#[derive(Clone, Debug, Default)]
pub struct EncodingOptions {
  pub(crate) encoding: EncodingType,
}

impl EncodingOptions {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn with_encoding(encoding: EncodingType) -> Self {
    Self {
      encoding,
    }
  }
}
