use serde::de::{Deserializer, DeserializeSeed, EnumAccess, Error, IntoDeserializer, VariantAccess, Visitor};

use node_types::StandardType;

pub struct Custom<D> {
  de: D,
  node_type: StandardType,
}

impl<D> Custom<D> {
  pub fn new(de: D, node_type: StandardType) -> Self {
    trace!("Custom::new(node_type: {:?})", node_type);

    Self { de, node_type }
  }
}

impl<'de, D> EnumAccess<'de> for Custom<D>
  where D: Deserializer<'de>
{
  type Error = D::Error;
  type Variant = Self;

  fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), D::Error>
    where V: DeserializeSeed<'de>
  {
    trace!("<Custom as EnumAccess>::variant_seed(node_type: {:?})", self.node_type);
    let variant = self.node_type.id.into_deserializer();
    seed.deserialize(variant).map(|s| (s, self))
  }
}

impl<'de, D> VariantAccess<'de> for Custom<D>
  where D: Deserializer<'de>
{
  type Error = D::Error;

  fn unit_variant(self) -> Result<(), D::Error> {
    Err(D::Error::custom("unit variant not supported"))
  }

  // Used to get the value the `Visitor` wants through the `DeserializeSeed`
  fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, D::Error>
    where T: DeserializeSeed<'de>
  {
    trace!("<Custom as VariantAccess>::newtype_variant_seed()");
    seed.deserialize(self.de)
  }

  fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value, D::Error>
    where V: Visitor<'de>
  {
    Err(D::Error::custom("tuple variant not supported"))
  }

  fn struct_variant<V>(self, _fields: &'static [&'static str], _visitor: V) -> Result<V::Value, D::Error>
    where V: Visitor<'de>
  {
    Err(D::Error::custom("struct variant not supported"))
  }
}

impl<'de, D> Deserializer<'de> for Custom<D>
  where D: Deserializer<'de>
{
  type Error = D::Error;

  #[inline]
  fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, D::Error>
    where V: Visitor<'de>
  {
    trace!("<Custom as Deserializer>::deserialize_any(node_type: {:?})", self.node_type);
    visitor.visit_enum(self)
  }

  forward_to_deserialize_any! {
    bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str
    string bytes byte_buf option unit unit_struct newtype_struct seq
    tuple tuple_struct map struct enum identifier ignored_any
  }
}
