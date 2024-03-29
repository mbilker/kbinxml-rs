use std::io::Read;
use std::net::Ipv4Addr;

use byteorder::{BigEndian, ReadBytesExt};
use bytes::BufMut;
use snafu::ResultExt;

use crate::error::*;

pub trait IntoKbinBytes {
    fn write_kbin_bytes<B: BufMut>(self, buf: &mut B);
}

pub trait FromKbinBytes: Sized {
    fn from_kbin_bytes<R: Read>(input: &mut R) -> Result<Self>;
}

impl IntoKbinBytes for i8 {
    fn write_kbin_bytes<B: BufMut>(self, buf: &mut B) {
        buf.put_i8(self);
    }
}

impl FromKbinBytes for i8 {
    fn from_kbin_bytes<R: Read>(input: &mut R) -> Result<Self> {
        input.read_i8().context(DataConvertSnafu)
    }
}

impl IntoKbinBytes for u8 {
    fn write_kbin_bytes<B: BufMut>(self, buf: &mut B) {
        buf.put_u8(self);
    }
}

impl FromKbinBytes for u8 {
    fn from_kbin_bytes<R: Read>(input: &mut R) -> Result<Self> {
        input.read_u8().context(DataConvertSnafu)
    }
}

impl IntoKbinBytes for bool {
    fn write_kbin_bytes<B: BufMut>(self, buf: &mut B) {
        buf.put_u8(if self { 0x01 } else { 0x00 })
    }
}

impl FromKbinBytes for bool {
    fn from_kbin_bytes<R: Read>(input: &mut R) -> Result<Self> {
        match u8::from_kbin_bytes(input)? {
            0x00 => Ok(false),
            0x01 => Ok(true),
            input => Err(KbinError::InvalidBooleanInput { input }),
        }
    }
}

impl<'a> IntoKbinBytes for &'a [u8] {
    fn write_kbin_bytes<B: BufMut>(self, buf: &mut B) {
        buf.put(self);
    }
}

impl IntoKbinBytes for Ipv4Addr {
    fn write_kbin_bytes<B: BufMut>(self, buf: &mut B) {
        let octets = self.octets();

        buf.put(&octets[..])
    }
}

impl FromKbinBytes for Ipv4Addr {
    fn from_kbin_bytes<R: Read>(input: &mut R) -> Result<Self> {
        let mut octets = [0; 4];
        input.read_exact(&mut octets).context(DataConvertSnafu)?;

        Ok(Ipv4Addr::from(octets))
    }
}

macro_rules! multibyte_impl {
    (
      $(($type:ty, $write_method:ident, $read_method:ident)),*$(,)?
    ) => {
        $(
            impl IntoKbinBytes for $type {
                fn write_kbin_bytes<B: BufMut>(self, buf: &mut B) {
                    buf.$write_method(self);
                }
            }

            impl FromKbinBytes for $type {
                fn from_kbin_bytes<R: Read>(input: &mut R) -> Result<Self> {
                    input.$read_method::<BigEndian>().context(DataConvertSnafu)
                }
            }
        )*
    };
}

macro_rules! tuple_impl {
    (
        i8: [$($i8_count:expr),*],
        u8: [$($u8_count:expr),*],
        bool: [$($bool_count:expr),*],
        multi: [
            $([$type:ty ; $($count:expr),*] => ($write_method:ident, $read_method:ident)),*$(,)?
        ]
    ) => {
        $(
            impl<'a> IntoKbinBytes for &'a [i8; $i8_count] {
                fn write_kbin_bytes<B: BufMut>(self, buf: &mut B) {
                    for value in self.into_iter() {
                        buf.put_i8(*value);
                    }
                }
            }

            impl FromKbinBytes for [i8; $i8_count] {
                fn from_kbin_bytes<R: Read>(input: &mut R) -> Result<Self> {
                    let mut values = Self::default();
                    input.read_i8_into(&mut values).context(DataConvertSnafu)?;

                    Ok(values)
                }
            }
        )*
        $(
            impl<'a> IntoKbinBytes for &'a [u8; $u8_count] {
                fn write_kbin_bytes<B: BufMut>(self, buf: &mut B) {
                    buf.put_slice(&self[..]);
                }
            }

            impl FromKbinBytes for [u8; $u8_count] {
                fn from_kbin_bytes<R: Read>(input: &mut R) -> Result<Self> {
                    let mut values = Self::default();
                    input.read_exact(&mut values).context(DataConvertSnafu)?;

                    Ok(values)
                }
            }
        )*
        $(
            impl<'a> IntoKbinBytes for &'a [bool; $bool_count] {
                fn write_kbin_bytes<B: BufMut>(self, buf: &mut B) {
                    for value in self.into_iter() {
                        value.write_kbin_bytes(buf);
                    }
                }
            }

            impl FromKbinBytes for [bool; $bool_count] {
                fn from_kbin_bytes<R: Read>(input: &mut R) -> Result<Self> {
                    let mut values = Self::default();

                    for i in 0..$bool_count {
                        values[i] = bool::from_kbin_bytes(input)?;
                    }

                    Ok(values)
                }
            }
        )*
        $(
            $(
                impl<'a> IntoKbinBytes for &'a [$type; $count] {
                    fn write_kbin_bytes<B: BufMut>(self, buf: &mut B) {
                        for value in self.into_iter() {
                            buf.$write_method(*value);
                        }
                    }
                }

                impl FromKbinBytes for [$type; $count] {
                    fn from_kbin_bytes<R: Read>(input: &mut R) -> Result<Self> {
                        let mut values = Self::default();
                        input.$read_method::<BigEndian>(&mut values).context(DataConvertSnafu)?;

                        Ok(values)
                    }
                }
            )*
        )*
    };
}

multibyte_impl! {
    (i16, put_i16, read_i16),
    (u16, put_u16, read_u16),
    (i32, put_i32, read_i32),
    (u32, put_u32, read_u32),
    (i64, put_i64, read_i64),
    (u64, put_u64, read_u64),
    (f32, put_f32, read_f32),
    (f64, put_f64, read_f64),
}

tuple_impl! {
    i8: [2, 3, 4, 16],
    u8: [2, 3, 4, 16],
    bool: [2, 3, 4, 16],
    multi: [
        [i16; 2, 3, 4, 8] => (put_i16, read_i16_into),
        [u16; 2, 3, 4, 8] => (put_u16, read_u16_into),
        [i32; 2, 3, 4] => (put_i32, read_i32_into),
        [u32; 2, 3, 4] => (put_u32, read_u32_into),
        [i64; 2, 3, 4] => (put_i64, read_i64_into),
        [u64; 2, 3, 4] => (put_u64, read_u64_into),
        [f32; 2, 3, 4] => (put_f32, read_f32_into),
        [f64; 2, 3, 4] => (put_f64, read_f64_into),
    ]
}
