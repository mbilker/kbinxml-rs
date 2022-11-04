use crate::compression_type::CompressionType;
use crate::encoding_type::EncodingType;

#[derive(Clone, Debug, Default)]
pub struct Options {
    pub(crate) compression: CompressionType,
    pub(crate) encoding: EncodingType,
}

#[derive(Default)]
pub struct OptionsBuilder {
    compression: CompressionType,
    encoding: EncodingType,
}

impl Options {
    pub fn new(compression: CompressionType, encoding: EncodingType) -> Self {
        Self {
            compression,
            encoding,
        }
    }

    pub fn builder() -> OptionsBuilder {
        OptionsBuilder::default()
    }

    pub fn with_encoding(encoding: EncodingType) -> Self {
        Self {
            encoding,
            ..Default::default()
        }
    }
}

impl OptionsBuilder {
    pub fn compression(&mut self, compression: CompressionType) -> &mut Self {
        self.compression = compression;
        self
    }

    pub fn encoding(&mut self, encoding: EncodingType) -> &mut Self {
        self.encoding = encoding;
        self
    }

    pub fn build(&self) -> Options {
        Options {
            compression: self.compression,
            encoding: self.encoding,
        }
    }
}
