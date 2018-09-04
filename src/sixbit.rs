use std::collections::HashMap;
use std::io::{Read, Write};

use byteorder::{ReadBytesExt, WriteBytesExt};
use failure::ResultExt;
use num::{BigUint, FromPrimitive, ToPrimitive};

use error::{KbinError, KbinErrorKind};

static CHAR_MAP: &'static [u8] = b"0123456789:ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdefghijklmnopqrstuvwxyz";

lazy_static! {
  static ref BYTE_MAP: HashMap<u8, u8> = {
    CHAR_MAP
      .iter()
      .enumerate()
      .map(|(i, value)| {
        (*value, i as u8)
      })
      .collect()
  };
}

pub type SixbitSize = (u8, usize, usize);

pub struct Sixbit;

impl Sixbit {
  pub fn size<T>(reader: &mut T) -> Result<SixbitSize, KbinError>
    where T: Read
  {
    let len = reader.read_u8().context(KbinErrorKind::SixbitLengthRead)?;
    let real_len = (f32::from(len * 6) / 8f32).ceil();
    let real_len = (real_len as u32) as usize;
    let padding = (8 - ((len * 6) % 8)) as usize;
    let padding = if padding == 8 { 0 } else { padding };
    debug!("sixbit_len: {}, real_len: {}, padding: {}", len, real_len, padding);

    Ok((len, real_len, padding))
  }

  pub fn pack<T>(writer: &mut T, input: &str) -> Result<(), KbinError>
    where T: Write
  {
    let sixbit_chars = input
      .bytes()
      .map(|ch| {
        *BYTE_MAP.get(&ch).expect("Character must be a valid sixbit character")
      });
    let len = input.len() as usize;
    let padding = 8 - len * 6 % 8;
    let padding = if padding == 8 { 0 } else { padding };
    let real_len = (len * 6 + padding) / 8;
    debug!("sixbit_len: {}, real_len: {}, padding: {}", len, real_len, padding);

    let mut bits = BigUint::new(vec![0; real_len]);
    for ch in sixbit_chars {
      bits <<= 6;
      bits |= BigUint::from_u8(ch).unwrap();
    }
    bits <<= padding;

    let bytes = bits.to_bytes_be();
    writer.write_u8(len as u8).context(KbinErrorKind::SixbitLengthWrite)?;
    writer.write(&bytes).context(KbinErrorKind::SixbitWrite)?;

    Ok(())
  }

  pub fn unpack<T>(reader: &mut T) -> Result<String, KbinError>
    where T: Read
  {
    let (sixbit_len, len, padding) = Sixbit::size(reader)?;

    let mut buf = vec![0; len];
    reader.read_exact(&mut buf).context(KbinErrorKind::SixbitRead)?;

    let bits = BigUint::from_bytes_be(&buf);
    let bits = bits >> padding;
    debug!("bits: 0b{:b}", bits);

    let mask = BigUint::from_u8(0b111111).unwrap();
    let result = (1..=sixbit_len).map(|i| {
      // Get the current sixbit part starting from the the left most bit in
      // big endian order
      let shift = ((sixbit_len - i) * 6) as usize;
      let bits = bits.clone();
      let mask = mask.clone();
      let current = (bits >> shift) & mask;

      CHAR_MAP[current.to_usize().unwrap()] as char
    }).collect();

    debug!("result: {}", result);
    Ok(result)
  }
}
