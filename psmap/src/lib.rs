use kbinxml::KbinError;
use thiserror::Error;

// Re-export proc macro
pub use psmap_derive::psmap;

#[derive(Debug, Error)]
pub enum PsmapError {
    #[error("Attribute `{attribute}` not found in `{source_name}` for `{struct_name}`")]
    AttributeNotFound {
        attribute: &'static str,
        source_name: &'static str,
        struct_name: &'static str,
    },

    #[error("Failed to parse attribute `{attribute}` in `{source_name}` for `{struct_name}`")]
    AttributeParse {
        attribute: &'static str,
        source_name: &'static str,
        struct_name: &'static str,
        source: KbinError,
    },

    #[error("Field `{target}` not found for `{struct_name}`")]
    FieldNotFound {
        target: &'static str,
        struct_name: &'static str,
    },

    #[error("Field `{target}` not found in `{source_name}` for `{struct_name}`")]
    FieldNotFoundFromSource {
        target: &'static str,
        source_name: &'static str,
        struct_name: &'static str,
    },

    #[error("Node field `{source_name}` does not have a value")]
    ValueNotFound {
        source_name: &'static str,
    },
}
