trait KbinWrapperType<T> {
  fn from_kbin_bytes(input: &[u8]) -> String;
}

macro_rules! number_impl {
  (integer; $($inner_type:ident),*) => {
    $(
      impl KbinWrapperType<$inner_type> for $inner_type {
        fn from_kbin_bytes(input: &[u8]) -> String {
          println!("KbinWrapperType<{}> => input: {:02x?}", stringify!($inner_type), input);
          //String::from(concat!("integer ", stringify!($inner_type)))

          let mut data = [0; ::std::mem::size_of::<$inner_type>()];
          data.clone_from_slice(input);
          format!("{}", $inner_type::from_be($inner_type::from_bytes(data)))
        }
      }
    )*
  };
  (float; $($intermediate:ident => $inner_type:ident),*) => {
    $(
      impl KbinWrapperType<$inner_type> for $inner_type {
        fn from_kbin_bytes(input: &[u8]) -> String {
          println!("KbinWrapperType<{}> => input: {:02x?}", stringify!($inner_type), input);
          //String::from(concat!("float ", stringify!($inner_type)))

          let mut data = [0; ::std::mem::size_of::<$inner_type>()];
          data.clone_from_slice(input);
          let bits = $intermediate::from_be($intermediate::from_bytes(data));

          format!("{:.6}", $inner_type::from_bits(bits))
        }
      }
    )*
  };
}

number_impl!(integer; i8, u8, i16, u16, i32, u32, i64, u64);
number_impl!(float; u32 => f32, u64 => f64);

impl KbinWrapperType<bool> for bool {
  fn from_kbin_bytes(input: &[u8]) -> String {
    println!("KbinWrapperType<bool> => input: {:02x?}", input);
    //String::from("bool")

    let value = match input[0] {
      0x00 => "0",
      0x01 => "1",
      v => panic!("Unsupported value for boolean: {}", v),
    };
    String::from(value)
  }
}

struct DummyConverter;
impl KbinWrapperType<DummyConverter> for DummyConverter {
  fn from_kbin_bytes(_input: &[u8]) -> String {
    String::from("")
  }
}

struct InvalidConverter;
impl KbinWrapperType<InvalidConverter> for InvalidConverter {
  fn from_kbin_bytes(input: &[u8]) -> String {
    panic!("Invalid kbin type converter called for input: {:02x?}", input);
  }
}

macro_rules! construct_types {
  (
    $(
      ($id:expr, $konst:ident, $name:expr, $alt_name:expr, $size:expr, $count:expr, $inner_type:ident);
    )+
  ) => {
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
    pub enum KbinType {
      $(
        $konst,
      )+
    }

    impl KbinType {
      pub fn from_u8(input: u8) -> KbinType {
        match input {
          $(
            $id => KbinType::$konst,
          )+
          _ => panic!("Node type {} not implemented", input),
        }
      }

      pub fn name(&self) -> &'static str {
        match *self {
          $(
            KbinType::$konst => $name,
          )+
        }
      }

      #[allow(dead_code)]
      pub fn alt_name(&self) -> Option<&'static str> {
        match *self {
          $(
            KbinType::$konst => $alt_name,
          )+
        }
      }

      pub fn size(&self) -> i8 {
        match *self {
          $(
            KbinType::$konst => $size,
          )+
        }
      }

      pub fn count(&self) -> i8 {
        match *self {
          $(
            KbinType::$konst => $count,
          )+
        }
      }

      pub fn parse_bytes(&self, input: &[u8]) -> String {
        match *self {
          $(
            KbinType::$konst => {
              if $count == -1 {
                panic!("Tried to parse special type: {}", self.name());
              } else if $count == 0 {
                String::new()
              } else if $count == 1 {
                $inner_type::from_kbin_bytes(input)
              } else if $count > 1 {
                String::new()
              } else {
                unimplemented!();
              }
            },
          )+
        }
      }
    }
  }
}

construct_types! {
  ( 2, S8,       "s8",     None,           1, 1, i8);
  ( 3, U8,       "u8",     None,           1, 1, u8);
  ( 4, S16,      "s16",    None,           2, 1, i16);
  ( 5, U16,      "u16",    None,           2, 1, u16);
  ( 6, S32,      "s32",    None,           4, 1, i32);
  ( 7, U32,      "u32",    None,           4, 1, u32);
  ( 8, S64,      "s64",    None,           8, 1, i64);
  ( 9, U64,      "u64",    None,           8, 1, u64);
  (10, Binary,   "bin",    Some("binary"), 1, -1, DummyConverter);
  (11, String,   "str",    Some("string"), 1, -1, DummyConverter);
  (12, Ip4,      "ip4",    None,           1, 4, DummyConverter); // TODO: implement IP address parsing
  (13, Time,     "time",   None,           4, 1, u32);
  (14, Float,    "float",  Some("f"),      4, 1, f32);
  (15, Double,   "double", Some("d"),      8, 1, f64);
  (16, S8_2,     "2s8",    None,           1, 2, i8);
  (17, U8_2,     "2u8",    None,           1, 2, u8);
  (18, S16_2,    "2s16",   None,           2, 2, i16);
  (19, U16_2,    "2u16",   None,           2, 2, u16);
  (20, S32_2,    "2s32",   None,           4, 2, i32);
  (21, U32_2,    "2u32",   None,           4, 2, u32);
  (22, S64_2,    "2s64",   Some("vs64"),   8, 2, i64);
  (23, U64_2,    "2u64",   Some("vu64"),   8, 2, u64);
  (24, Float2,   "2f",     None,           4, 2, f32);
  (25, Double2,  "2d",     Some("vd"),     8, 2, f64);
  (26, S8_3,     "3s8",    None,           1, 3, i8);
  (27, U8_3,     "3u8",    None,           1, 3, u8);
  (28, S16_3,    "3s16",   None,           2, 3, i16);
  (29, U16_3,    "3u16",   None,           2, 3, u16);
  (30, S32_3,    "3s32",   None,           4, 3, i32);
  (31, U32_3,    "3u32",   None,           4, 3, u32);
  (32, S64_3,    "3s64",   None,           8, 3, i64);
  (33, U64_3,    "3u64",   None,           8, 3, u64);
  (34, Float3,   "3f",     None,           4, 3, f32);
  (35, Double3,  "3d",     None,           8, 3, f64);
  (36, S8_4,     "4s8",    None,           1, 4, i8);
  (37, U8_4,     "4u8",    None,           1, 4, u8);
  (38, S16_4,    "4s16",   None,           2, 4, i16);
  (39, U16_4,    "4u16",   None,           2, 4, u16);
  (40, S32_4,    "4s32",   Some("vs32"),   4, 4, i32);
  (41, U32_4,    "4u32",   Some("vu32"),   4, 4, u32);
  (42, S64_4,    "4s64",   None,           8, 4, i64);
  (43, U64_4,    "4u64",   None,           8, 4, u64);
  (44, Float4,   "4f",     Some("vf"),     4, 4, f32);
  (45, Double4,  "4d",     None,           8, 4, f64);
  // 46 = Attribute
  // no 47
  (48, Vs8,      "vs8",    None,           1, 16, i8);
  (49, Vu8,      "vu8",    None,           1, 16, u8);
  (50, Vs16,     "vs16",   None,           1, 8, i16);
  (51, Vu16,     "vu16",   None,           1, 8, u16);
  (52, Boolean,  "bool",   Some("b"),      1, 1, bool);
  (53, Boolean2, "2b",     None,           1, 2, bool);
  (54, Boolean3, "3b",     None,           1, 3, bool);
  (55, Boolean4, "4b",     None,           1, 4, bool);
  (56, Vb,       "vb",     None,           1, 16, bool);

  ( 1, NodeStart, "void", None, 0, 0, InvalidConverter);
  (46, Attribute, "attr", None, 0, 0, InvalidConverter);

  (190, NodeEnd, "nodeEnd", None, 0, 0, InvalidConverter);
  (191, FileEnd, "fileEnd", None, 0, 0, InvalidConverter);
}
