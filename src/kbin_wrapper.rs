use std::fmt::Write;

use byteorder::WriteBytesExt;
use failure::ResultExt;

use error::{KbinError, KbinErrorKind};

pub(crate) trait KbinWrapperType<T> {
  fn from_kbin_bytes(output: &mut String, input: &[u8]) -> Result<(), KbinError>;
  fn to_kbin_bytes(output: &mut Vec<u8>, input: &str) -> Result<(), KbinError>;
}

macro_rules! number_impl {
  (int; $($inner_type:ident),*) => {
    $(
      impl KbinWrapperType<$inner_type> for $inner_type {
        fn from_kbin_bytes(output: &mut String, input: &[u8]) -> Result<(), KbinError> {
          trace!("KbinWrapperType<{}> from bytes => input: {:02x?}", stringify!($inner_type), input);

          let mut data = [0; ::std::mem::size_of::<$inner_type>()];
          data.clone_from_slice(input);
          write!(output, "{}", $inner_type::from_be_bytes(data))
            .context(KbinErrorKind::ByteParse(stringify!($inner_type)))?;

          Ok(())
        }

        fn to_kbin_bytes(output: &mut Vec<u8>, input: &str) -> Result<(), KbinError> {
          let num = input.parse::<$inner_type>().context(KbinErrorKind::StringParse(stringify!($inner_type)))?;
          trace!("KbinWrapperType<{}> to bytes => input: '{}', output: {}", stringify!($inner_type), input, num);

          let data = $inner_type::to_be_bytes(num);
          output.extend_from_slice(&data);

          Ok(())
        }
      }
    )*
  };
  (float; $($intermediate:ident => $inner_type:ident),*) => {
    $(
      impl KbinWrapperType<$inner_type> for $inner_type {
        fn from_kbin_bytes(output: &mut String, input: &[u8]) -> Result<(), KbinError> {
          trace!("KbinWrapperType<{}> from bytes => input: {:02x?}", stringify!($inner_type), input);

          let mut data = [0; ::std::mem::size_of::<$inner_type>()];
          data.clone_from_slice(input);
          let bits = $intermediate::from_be_bytes(data);

          write!(output, "{:.6}", $inner_type::from_bits(bits))
            .context(KbinErrorKind::ByteParse(stringify!($inner_type)))?;

          Ok(())
        }

        fn to_kbin_bytes(output: &mut Vec<u8>, input: &str) -> Result<(), KbinError> {
          let num = input.parse::<$inner_type>().context(KbinErrorKind::StringParse(stringify!($inner_type)))?;
          trace!("KbinWrapperType<{}> to bytes => input: '{}', output: {}", stringify!($inner_type), input, num);

          let data = $intermediate::to_be_bytes(num.to_bits());
          output.extend_from_slice(&data);

          Ok(())
        }
      }
    )*
  };
}

number_impl!(int; u8, u16, u32, u64);
number_impl!(int; i8, i16, i32, i64);
number_impl!(float; u32 => f32, u64 => f64);

impl KbinWrapperType<bool> for bool {
  fn from_kbin_bytes(output: &mut String, input: &[u8]) -> Result<(), KbinError> {
    trace!("KbinWrapperType<bool> from bytes => input: {:02x?}", input);

    let value = match input[0] {
      0x00 => "0",
      0x01 => "1",
      v => panic!("Unsupported value for boolean: {}", v),
    };
    output.push_str(value);

    Ok(())
  }

  fn to_kbin_bytes(output: &mut Vec<u8>, input: &str) -> Result<(), KbinError> {
    let value = match input {
      "0" => 0x00,
      "1" => 0x01,
      v => panic!("Unsupported value for boolean: {}", v),
    };

    trace!("KbinWrapperType<bool> to bytes => input: '{}', output: {}", input, value);
    output.write_u8(value).context(KbinErrorKind::DataWrite("bool"))?;

    Ok(())
  }
}

pub(crate) struct Ip4;
pub(crate) struct DummyConverter;
pub(crate) struct InvalidConverter;

impl KbinWrapperType<Ip4> for Ip4 {
  fn from_kbin_bytes(output: &mut String, input: &[u8]) -> Result<(), KbinError> {
    trace!("KbinWrapperType<Ip4> from bytes => input: {:02x?}", input);

    if input.len() != 4 {
      panic!("Ip4 type requires exactly 4 bytes of data, input: {:02x?}", input);
    }

    write!(output, "{}.{}.{}.{}", input[0], input[1], input[2], input[3])
      .context(KbinErrorKind::ByteParse("Ip4"))?;

    Ok(())
  }

  fn to_kbin_bytes(output: &mut Vec<u8>, input: &str) -> Result<(), KbinError> {
    trace!("KbinWrapperType<Ip4> to bytes => input: '{}'", input);

    for part in input.split('.') {
      let num = part.parse::<u8>().context(KbinErrorKind::StringParse("ip4 segment"))?;
      output.write_u8(num).context(KbinErrorKind::DataWrite("ip4"))?;
    }

    Ok(())
  }
}

impl KbinWrapperType<DummyConverter> for DummyConverter {
  fn from_kbin_bytes(_output: &mut String, _input: &[u8]) -> Result<(), KbinError> { Ok(()) }
  fn to_kbin_bytes(_output: &mut Vec<u8>, _input: &str) -> Result<(), KbinError> { Ok(()) }
}

impl KbinWrapperType<InvalidConverter> for InvalidConverter {
  fn from_kbin_bytes(_output: &mut String, input: &[u8]) -> Result<(), KbinError> {
    panic!("Invalid kbin type converter called for input: {:02x?}", input);
  }

  fn to_kbin_bytes(_output: &mut Vec<u8>, input: &str) -> Result<(), KbinError> {
    panic!("Invalid kbin type converter called for input: {}", input);
  }
}
