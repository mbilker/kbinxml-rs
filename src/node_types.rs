use std::error::Error;
use std::fmt;
use std::ops::Deref;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct KbinType {
    pub id: u8,
    pub konst: &'static str,
    pub name: &'static str,
    pub alt_name: Option<&'static str>,
    pub size: usize,
    pub count: usize,
}

#[derive(Debug)]
pub enum UnknownKbinType {
    Byte(u8),
    Name(String),
}

impl fmt::Display for KbinType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} ({})", self.konst, self.name)
    }
}

impl fmt::Display for UnknownKbinType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Byte(byte) => write!(f, "Unknown or not implemented type: {}", byte),
            Self::Name(name) => write!(f, "Unknown or not implemented name: {}", name),
        }
    }
}

impl Error for UnknownKbinType {}

macro_rules! construct_types {
  (
    $(
      ($id:expr, $upcase:ident, $konst:ident, $name:expr, $alt_name:expr, $size:expr, $count:expr);
    )+
  ) => {
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
    pub enum StandardType {
      $(
        $konst = $id,
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
      pub fn from_u8(input: u8) -> Result<StandardType, UnknownKbinType> {
        match input {
          $(
            $id => Ok(StandardType::$konst),
          )+
          _ => Err(UnknownKbinType::Byte(input)),
        }
      }

      pub fn from_name(input: &str) -> Result<StandardType, UnknownKbinType> {
        match input {
          $(
            $name => Ok(StandardType::$konst),
          )+
          _ => Err(UnknownKbinType::Name(String::from(input))),
        }
      }
    }

    impl fmt::Display for StandardType {
      fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
          $(
            StandardType::$konst => f.write_str(stringify!($konst)),
          )+
        }
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
  ( 2, S8,       S8,       "s8",     None,           1, 1);
  ( 3, U8,       U8,       "u8",     None,           1, 1);
  ( 4, S16,      S16,      "s16",    None,           2, 1);
  ( 5, U16,      U16,      "u16",    None,           2, 1);
  ( 6, S32,      S32,      "s32",    None,           4, 1);
  ( 7, U32,      U32,      "u32",    None,           4, 1);
  ( 8, S64,      S64,      "s64",    None,           8, 1);
  ( 9, U64,      U64,      "u64",    None,           8, 1);
  (10, BINARY,   Binary,   "bin",    Some("binary"), 1, 0);
  (11, STRING,   String,   "str",    Some("string"), 1, 0);
  (12, IP4,      Ip4,      "ip4",    None,           4, 1); // Using size of 4 rather than count of 4
  (13, TIME,     Time,     "time",   None,           4, 1);
  (14, FLOAT,    Float,    "float",  Some("f"),      4, 1);
  (15, DOUBLE,   Double,   "double", Some("d"),      8, 1);
  (16, S8_2,     S8_2,     "2s8",    None,           1, 2);
  (17, U8_2,     U8_2,     "2u8",    None,           1, 2);
  (18, S16_2,    S16_2,    "2s16",   None,           2, 2);
  (19, U16_2,    U16_2,    "2u16",   None,           2, 2);
  (20, S32_2,    S32_2,    "2s32",   None,           4, 2);
  (21, U32_2,    U32_2,    "2u32",   None,           4, 2);
  (22, S64_2,    S64_2,    "2s64",   Some("vs64"),   8, 2);
  (23, U64_2,    U64_2,    "2u64",   Some("vu64"),   8, 2);
  (24, FLOAT_2,  Float2,   "2f",     None,           4, 2);
  (25, DOUBLE_2, Double2,  "2d",     Some("vd"),     8, 2);
  (26, S8_3,     S8_3,     "3s8",    None,           1, 3);
  (27, U8_3,     U8_3,     "3u8",    None,           1, 3);
  (28, S16_3,    S16_3,    "3s16",   None,           2, 3);
  (29, U16_3,    U16_3,    "3u16",   None,           2, 3);
  (30, S32_3,    S32_3,    "3s32",   None,           4, 3);
  (31, U32_3,    U32_3,    "3u32",   None,           4, 3);
  (32, S64_3,    S64_3,    "3s64",   None,           8, 3);
  (33, U64_3,    U64_3,    "3u64",   None,           8, 3);
  (34, FLOAT_3,  Float3,   "3f",     None,           4, 3);
  (35, DOUBLE_3, Double3,  "3d",     None,           8, 3);
  (36, S8_4,     S8_4,     "4s8",    None,           1, 4);
  (37, U8_4,     U8_4,     "4u8",    None,           1, 4);
  (38, S16_4,    S16_4,    "4s16",   None,           2, 4);
  (39, U16_4,    U16_4,    "4u16",   None,           2, 4);
  (40, S32_4,    S32_4,    "4s32",   Some("vs32"),   4, 4);
  (41, U32_4,    U32_4,    "4u32",   Some("vu32"),   4, 4);
  (42, S64_4,    S64_4,    "4s64",   None,           8, 4);
  (43, U64_4,    U64_4,    "4u64",   None,           8, 4);
  (44, FLOAT_4,  Float4,   "4f",     Some("vf"),     4, 4);
  (45, DOUBLE_4, Double4,  "4d",     None,           8, 4);
  // 46 = Attribute
  // no 47
  (48, VS8,      Vs8,      "vs8",    None,           1, 16);
  (49, VU8,      Vu8,      "vu8",    None,           1, 16);
  (50, VS16,     Vs16,     "vs16",   None,           2, 8);
  (51, VU16,     Vu16,     "vu16",   None,           2, 8);
  (52, BOOL,     Boolean,  "bool",   Some("b"),      1, 1);
  (53, BOOL_2,   Boolean2, "2b",     None,           1, 2);
  (54, BOOL_3,   Boolean3, "3b",     None,           1, 3);
  (55, BOOL_4,   Boolean4, "4b",     None,           1, 4);
  (56, VB,       Vb,       "vb",     None,           1, 16);

  ( 1, NODE_START, NodeStart, "void", None, 0, 0);
  (46, ATTRIBUTE,  Attribute, "attr", None, 0, 0);

  (190, NODE_END, NodeEnd, "nodeEnd", None, 0, 0);
  (191, FILE_END, FileEnd, "fileEnd", None, 0, 0);
}
