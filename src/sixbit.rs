use std::collections::HashMap;
use std::io::{Read, Write};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use num::{BigUint, FromPrimitive, ToPrimitive};

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

#[allow(dead_code)]
pub fn pack_sixbit<T>(writer: &mut T, input: &str)
  where T: Write
{
  let sixbit_chars = input
    .bytes()
    .map(|ch| {
      *BYTE_MAP.get(&ch).expect("Character must be a valid sixbit character")
    });
  let padding = 8 - input.len() * 6 % 8;
  let padding = if padding == 8 { 0 } else { padding };

  let mut bits = 0;
  for ch in sixbit_chars {
    bits <<= 6;
    bits |= ch as u64;
  }
  bits <<= padding;

  let len = input.len() as u8;
  writer.write_u8(len).expect("Unable to write sixbit string length");
  writer.write_uint::<BigEndian>(bits, (input.len() * 6 + padding) / 8).expect("Unable to write sixbit contents");
}

pub fn unpack_sixbit<T>(reader: &mut T) -> String
  where T: Read
{
  let len = reader.read_u8().expect("Unable to read sixbit string length");
  let real_len = (f32::from(len * 6) / 8f32).ceil();
  let real_len = (real_len as u32) as usize;
  let padding = (8 - ((len * 6) % 8)) as usize;
  let padding = if padding == 8 { 0 } else { padding };
  debug!("sixbit_len: {}, real_len: {}, padding: {}", len, real_len, padding);

  let mut buf = vec![0; real_len];
  reader.read_exact(&mut buf).expect("Unable to read sixbit string content");

  let bits = BigUint::from_bytes_be(&buf);
  let bits = bits >> padding;
  debug!("bits: 0b{:b}", bits);

  let mask = BigUint::from_u8(0b111111).unwrap();
  let result = (1..=len).map(|i| {
    // Get the current sixbit part starting from the the left most bit in
    // big endian order
    let shift = ((len - i) * 6) as usize;
    let bits = bits.clone();
    let mask = mask.clone();
    let current = (bits >> shift) & mask;
    //println!("current: 0b{:b} ({})", current, current);

    let entry = CHAR_MAP[current.to_usize().unwrap()];
    //println!("entry: {} ({})", entry, entry as char);

    entry as char
  }).collect();

  debug!("result: {}", result);
  result
}
