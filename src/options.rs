use encoding_type::EncodingType;

#[derive(Clone, Debug, Default)]
pub struct Options {
  pub(crate) encoding: EncodingType,
}

impl Options {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn with_encoding(encoding: EncodingType) -> Self {
    Self {
      encoding,
    }
  }
}
