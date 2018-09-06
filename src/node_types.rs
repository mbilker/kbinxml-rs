use std::fmt;
use std::ops::Deref;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct KbinType {
  pub id: u8,
  pub konst: &'static str,
  pub name: &'static str,
  pub alt_name: Option<&'static str>,
  pub size: usize,
  pub count: usize
}

impl fmt::Display for KbinType {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{} ({})", self.konst, self.name)
  }
}

macro_rules! find_type {
  ($($base:ident => [$($size:tt $alternate:ident)+])+) => {
    pub fn find_type(base: StandardType, len: usize) -> StandardType {
      match base {
        $(
          StandardType::$base => match len {
            1 => StandardType::$base,
            $(
              $size => StandardType::$alternate,
            )+
            _ => panic!("Unsupported len, base: {:?}, len: {}", base, len),
          },
        )*
        _ => panic!("Unsupported base, base: {:?}, len: {}", base, len),
      }
    }
  };
}

macro_rules! construct_types {
  (
    $(
      ($id:expr, $upcase:ident, $konst:ident, $name:expr, $alt_name:expr, $size:expr, $count:expr, $inner_type:ident);
    )+
  ) => {
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
    pub enum StandardType {
      $(
        $konst,
      )+
    }

    $(
      #[allow(non_upper_case_globals)]
      pub const $konst: KbinType = KbinType {
        id: $id,
        konst: stringify!($konst),
        name: $name,
        alt_name: $alt_name,
        size: $size,
        count: $count,
      };
    )+

    impl StandardType {
      pub fn from_u8(input: u8) -> StandardType {
        match input {
          $(
            $id => StandardType::$konst,
          )+
          _ => panic!("Node type {} not implemented", input),
        }
      }

      pub fn from_name(input: &str) -> StandardType {
        match input {
          $(
            $name => StandardType::$konst,
          )+
          _ => panic!("Node name {} not implemented", input),
        }
      }

      find_type! {
        S8 => [ 2 S8_2 3 S8_3 4 S8_4 ]
        U8 => [ 2 U8_2 3 U8_3 4 U8_4 ]
        S16 => [ 2 S16_2 3 S16_3 4 S16_4 ]
        U16 => [ 2 U16_2 3 U16_3 4 U16_4 ]
        S32 => [ 2 S32_2 3 S32_3 4 S32_4 ]
        U32 => [ 2 U32_2 3 U32_3 4 U32_4 ]
        Float => [ 2 Float2 3 Float3 4 Float4 ]
        Double => [ 2 Double2 3 Double3 4 Double4 ]
        Boolean => [ 2 Boolean2 3 Boolean3 4 Boolean4 ]
      }
    }

    impl Deref for StandardType {
      type Target = KbinType;

      fn deref(&self) -> &KbinType {
        match *self {
          $(
            StandardType::$konst => &$konst,
          )+
        }
      }
    }
  }
}

construct_types! {
  ( 2, S8,       S8,       "s8",     None,           1, 1, i8);
  ( 3, U8,       U8,       "u8",     None,           1, 1, u8);
  ( 4, S16,      S16,      "s16",    None,           2, 1, i16);
  ( 5, U16,      U16,      "u16",    None,           2, 1, u16);
  ( 6, S32,      S32,      "s32",    None,           4, 1, i32);
  ( 7, U32,      U32,      "u32",    None,           4, 1, u32);
  ( 8, S64,      S64,      "s64",    None,           8, 1, i64);
  ( 9, U64,      U64,      "u64",    None,           8, 1, u64);
  (10, BINARY,   Binary,   "bin",    Some("binary"), 1, 0, DummyConverter);
  (11, STRING,   String,   "str",    Some("string"), 1, 0, DummyConverter);
  (12, IP4,      Ip4,      "ip4",    None,           4, 1, Ip4); // Using size of 4 rather than count of 4
  (13, TIME,     Time,     "time",   None,           4, 1, u32);
  (14, FLOAT,    Float,    "float",  Some("f"),      4, 1, f32);
  (15, DOUBLE,   Double,   "double", Some("d"),      8, 1, f64);
  (16, S8_2,     S8_2,     "2s8",    None,           1, 2, i8);
  (17, U8_2,     U8_2,     "2u8",    None,           1, 2, u8);
  (18, S16_2,    S16_2,    "2s16",   None,           2, 2, i16);
  (19, U16_2,    U16_2,    "2u16",   None,           2, 2, u16);
  (20, S32_2,    S32_2,    "2s32",   None,           4, 2, i32);
  (21, U32_2,    U32_2,    "2u32",   None,           4, 2, u32);
  (22, S64_2,    S64_2,    "2s64",   Some("vs64"),   8, 2, i64);
  (23, U64_2,    U64_2,    "2u64",   Some("vu64"),   8, 2, u64);
  (24, FLOAT_2,  Float2,   "2f",     None,           4, 2, f32);
  (25, DOUBLE_2, Double2,  "2d",     Some("vd"),     8, 2, f64);
  (26, S8_3,     S8_3,     "3s8",    None,           1, 3, i8);
  (27, U8_3,     U8_3,     "3u8",    None,           1, 3, u8);
  (28, S16_3,    S16_3,    "3s16",   None,           2, 3, i16);
  (29, U16_3,    U16_3,    "3u16",   None,           2, 3, u16);
  (30, S32_3,    S32_3,    "3s32",   None,           4, 3, i32);
  (31, U32_3,    U32_3,    "3u32",   None,           4, 3, u32);
  (32, S64_3,    S64_3,    "3s64",   None,           8, 3, i64);
  (33, U64_3,    U64_3,    "3u64",   None,           8, 3, u64);
  (34, FLOAT_3,  Float3,   "3f",     None,           4, 3, f32);
  (35, DOUBLE_3, Double3,  "3d",     None,           8, 3, f64);
  (36, S8_4,     S8_4,     "4s8",    None,           1, 4, i8);
  (37, U8_4,     U8_4,     "4u8",    None,           1, 4, u8);
  (38, S16_4,    S16_4,    "4s16",   None,           2, 4, i16);
  (39, U16_4,    U16_4,    "4u16",   None,           2, 4, u16);
  (40, S32_4,    S32_4,    "4s32",   Some("vs32"),   4, 4, i32);
  (41, U32_4,    U32_4,    "4u32",   Some("vu32"),   4, 4, u32);
  (42, S64_4,    S64_4,    "4s64",   None,           8, 4, i64);
  (43, U64_4,    U64_4,    "4u64",   None,           8, 4, u64);
  (44, FLOAT_4,  Float4,   "4f",     Some("vf"),     4, 4, f32);
  (45, DOUBLE_4, Double4,  "4d",     None,           8, 4, f64);
  // 46 = Attribute
  // no 47
  (48, VS8,      Vs8,      "vs8",    None,           1, 16, i8);
  (49, VU8,      Vu8,      "vu8",    None,           1, 16, u8);
  (50, VS16,     Vs16,     "vs16",   None,           2, 8, i16);
  (51, VU16,     Vu16,     "vu16",   None,           2, 8, u16);
  (52, BOOL,     Boolean,  "bool",   Some("b"),      1, 1, bool);
  (53, BOOL_2,   Boolean2, "2b",     None,           1, 2, bool);
  (54, BOOL_3,   Boolean3, "3b",     None,           1, 3, bool);
  (55, BOOL_4,   Boolean4, "4b",     None,           1, 4, bool);
  (56, VB,       Vb,       "vb",     None,           1, 16, bool);

  ( 1, NODE_START, NodeStart, "void", None, 0, 0, InvalidConverter);
  (46, ATTRIBUTE,  Attribute, "attr", None, 0, 0, InvalidConverter);

  (190, NODE_END, NodeEnd, "nodeEnd", None, 0, 0, InvalidConverter);
  (191, FILE_END, FileEnd, "fileEnd", None, 0, 0, InvalidConverter);
}
