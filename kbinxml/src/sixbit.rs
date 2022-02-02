use std::collections::HashMap;
use std::io::{self, Read, Write};

use byteorder::{ReadBytesExt, WriteBytesExt};
use snafu::{ResultExt, Snafu};

const CHAR_MAP: &[u8] = b"0123456789:ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdefghijklmnopqrstuvwxyz";

lazy_static! {
    static ref BYTE_MAP: HashMap<u8, u8> = {
        CHAR_MAP
            .iter()
            .enumerate()
            .map(|(i, value)| (*value, i as u8))
            .collect()
    };
}

#[derive(Debug, Snafu)]
pub enum SixbitError {
    #[snafu(display("Failed to read sixbit string length"))]
    LengthRead { source: io::Error },

    #[snafu(display("Failed to write sixbit string length"))]
    LengthWrite { source: io::Error },

    #[snafu(display(
        "Failed to read sixbit string data (expected: {} bytes, got: {} bytes)",
        expected,
        actual
    ))]
    DataRead { expected: usize, actual: usize },

    #[snafu(display("Failed to write sixbit string data"))]
    DataWrite { source: io::Error },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SixbitSize {
    pub sixbit_len: u8,
    pub real_len: usize,
}

pub struct Sixbit;

impl Sixbit {
    pub fn size<T>(reader: &mut T) -> Result<SixbitSize, SixbitError>
    where
        T: Read,
    {
        let sixbit_len = reader.read_u8().context(LengthReadSnafu)?;
        let real_len = (f32::from(sixbit_len * 6) / 8f32).ceil();
        let real_len = (real_len as u32) as usize;
        debug!("sixbit_len: {}, real_len: {}", sixbit_len, real_len);

        Ok(SixbitSize {
            sixbit_len,
            real_len,
        })
    }

    pub fn pack<T>(writer: &mut T, input: &str) -> Result<(), SixbitError>
    where
        T: Write,
    {
        let sixbit_chars = input.bytes().map(|ch| {
            *BYTE_MAP
                .get(&ch)
                .expect("Character must be a valid sixbit character")
        });

        let len = input.len();
        let real_len = (f64::from(len as u32 * 6) / 8f64).ceil() as usize;
        debug!("sixbit_len: {}, real_len: {}", len, real_len);

        let mut i = 0;
        let mut bytes = vec![0; real_len];
        for ch in sixbit_chars {
            for _ in 0..6 {
                // Some crazy math that works on a single bit at a time, but
                // it still performs better than a `BigUint` calculation
                bytes[i / 8] |= (ch >> (5 - (i % 6)) & 1) << (7 - (i % 8));
                i += 1;
            }
        }

        writer.write_u8(len as u8).context(LengthWriteSnafu)?;
        writer.write_all(&bytes).context(DataWriteSnafu)?;

        Ok(())
    }

    pub fn unpack(buf: &[u8], size: SixbitSize) -> Result<String, SixbitError> {
        let SixbitSize {
            sixbit_len,
            real_len,
        } = size;

        if buf.len() < real_len {
            return Err(SixbitError::DataRead {
                expected: real_len,
                actual: buf.len(),
            });
        }

        let sixbit_len = sixbit_len as usize;
        let mut result = String::with_capacity(sixbit_len);
        for i in 0..sixbit_len {
            let mut current = 0u8;
            for j in 0..6 {
                let k = (i * 6) + j;
                current |= (buf[k / 8] >> (7 - (k % 8)) & 1) << (5 - (k % 6));
            }
            result.push(CHAR_MAP[current as usize] as char);
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    extern crate test;

    use std::io::{Cursor, Seek, SeekFrom};

    use test::{black_box, Bencher};

    use super::Sixbit;

    const TEST1_STR: &str = "hello";
    const TEST1_BYTES: &[u8] = &[5, 182, 172, 113, 208];

    #[test]
    fn test_pack() {
        let mut data: Cursor<Vec<u8>> = Cursor::new(Vec::new());
        Sixbit::pack(&mut data, TEST1_STR).expect("Failed to pack sixbit");
        assert_eq!(data.into_inner(), TEST1_BYTES);
    }

    #[test]
    fn test_unpack() {
        let size = Sixbit::size(&mut Cursor::new(TEST1_BYTES))
            .expect("Failed to get size of sixbit string");
        let result =
            Sixbit::unpack(&TEST1_BYTES[1..], size).expect("Failed to unpack sixbit string");
        assert_eq!(result, TEST1_STR);
    }

    #[bench]
    fn bench_pack(b: &mut Bencher) {
        let mut data: Cursor<Vec<u8>> = Cursor::new(Vec::with_capacity(10));

        b.iter(|| {
            for _ in 0..100 {
                data.seek(SeekFrom::Start(0)).unwrap();
                black_box(Sixbit::pack(&mut data, TEST1_STR).unwrap());
            }
        });

        assert_eq!(data.into_inner(), TEST1_BYTES);
    }

    #[bench]
    fn bench_unpack(b: &mut Bencher) {
        b.iter(|| {
            for _ in 0..100 {
                let size = Sixbit::size(&mut Cursor::new(TEST1_BYTES))
                    .expect("Failed to get size of sixbit string");
                let result = Sixbit::unpack(&TEST1_BYTES[1..], size)
                    .expect("Failed to unpack sixbit string");
                black_box(result);
            }
        });
    }
}
