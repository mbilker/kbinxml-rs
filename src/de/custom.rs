use serde::de::{DeserializeSeed, EnumAccess, IntoDeserializer, VariantAccess, Visitor};

use de::{Deserializer, Result};
use error::Error;
use node_types::StandardType;

pub struct Custom<'a, 'de: 'a> {
  de: &'a mut Deserializer<'de>,
  node_type: StandardType,
}

impl<'de, 'a> Custom<'a, 'de> {
  pub fn new(de: &'a mut Deserializer<'de>, node_type: StandardType) -> Self {
    trace!("Custom::new(node_type: {:?})", node_type);

    Self { de, node_type }
  }
}

impl<'de, 'a> EnumAccess<'de> for Custom<'a, 'de> {
  type Error = Error;
  type Variant = Self;

  fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant)>
    where V: DeserializeSeed<'de>
  {
    trace!("<Custom as EnumAccess>::variant_seed(node_type: {:?})", self.node_type);
    let variant = self.node_type.id.into_deserializer();
    seed.deserialize(variant).map(|s| (s, self))
  }
}

impl<'de, 'a> VariantAccess<'de> for Custom<'a, 'de> {
  type Error = Error;

  fn unit_variant(self) -> Result<()> {
    Err(Error::Message("unit variant not supported".into()))
  }

  // Used to get the value the `Visitor` wants through the `DeserializeSeed`
  fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
    where T: DeserializeSeed<'de>
  {
    trace!("<Custom as VariantAccess>::newtype_variant_seed()");
    seed.deserialize(self.de)
  }

  fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    Err(Error::Message("tuple variant not supported".into()))
  }

  fn struct_variant<V>(self, _fields: &'static [&'static str], _visitor: V) -> Result<V::Value>
    where V: Visitor<'de>
  {
    Err(Error::Message("struct variant not supported".into()))
  }
}
