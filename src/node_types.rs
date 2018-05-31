//use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
pub struct NodeType {
  pub id: u8,
  pub name: &'static str,
  pub alt_name: Option<&'static str>,
  pub size: i32,
  pub count: i32,
}

impl NodeType {
  fn new(
    id: u8,
    name: &'static str,
    alt_name: Option<&'static str>,
    size: i32,
    count: i32,
  ) -> Self {
    Self { id, name, alt_name, size, count }
  }
}

macro_rules! construct_types {
  (
    $(
      ($id:expr, $konst:ident, $name:expr, $alt_name:expr, $size:expr, $count:expr, $handler:tt);
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
      pub fn as_node_type(&self) -> NodeType {
        match *self {
          $(
            KbinType::$konst => NodeType::new($id, $name, $alt_name, $size, $count),
          )+
        }
      }
    }

    /*
    lazy_static! {
      pub static ref BYTE_XML_MAPPING: HashMap<u8, KbinType> = {
        let mut map = HashMap::new();
        $(
          map.insert($id, KbinType::$konst);
        )+
        map
      };

      pub static ref XML_TYPES: HashMap<&'static str, KbinType> = {
        let mut map = HashMap::new();
        $(
          map.insert($name, KbinType::$konst);
        )+
        map
      };
    }
    */
  }
}

construct_types! {
  ( 2, S8,       "s8",     None,           1, 1, s8);
  ( 3, U8,       "u8",     None,           1, 1, u8);
  ( 4, S16,      "s16",    None,           2, 1, s16);
  ( 5, U16,      "u16",    None,           2, 1, u16);
  ( 6, S32,      "s32",    None,           4, 1, s32);
  ( 7, U32,      "u32",    None,           4, 1, u32);
  ( 8, S64,      "s64",    None,           8, 1, s64);
  ( 9, U64,      "u64",    None,           8, 1, u64);
  (10, Binary,   "bin",    Some("binary"), 1, 0, special);
  (11, String,   "str",    Some("string"), 1, 0, special);
  (12, Ip4,      "ip4",    None,           1, 4, special);
  (13, Time,     "time",   None,           4, 1, u32);
  (14, Float,    "float",  Some("f"),      4, 1, f32);
  (15, Double,   "double", Some("d"),     8, 1, f64);
  (16, S8_2,     "2s8",    None,          1, 2, s8);
  (17, U8_2,     "2u8",    None,          1, 2, u8);
  (18, S16_2,    "2s16",   None,          2, 2, s16);
  (19, U16_2,    "2u16",   None,          2, 2, u16);
  (20, S32_2,    "2s32",   None,          4, 2, s32);
  (21, U32_2,    "2u32",   None,          4, 2, u32);
  (22, S64_2,    "2s64",   Some("vs64"),  8, 2, s64);
  (23, U64_2,    "2u64",   Some("vu64"),  8, 2, u64);
  (24, Float2,   "2f",     None,          4, 2, f32);
  (25, Double2,  "2d",     Some("vd"),    8, 2, f64);
  (26, S8_3,     "3s8",    None,          1, 3, s8);
  (27, U8_3,     "3u8",    None,          1, 3, u8);
  (28, S16_3,    "3s16",   None,          2, 3, s16);
  (29, U16_3,    "3u16",   None,          2, 3, u16);
  (30, S32_3,    "3s32",   None,          4, 3, s32);
  (31, U32_3,    "3u32",   None,          4, 3, u32);
  (32, S64_3,    "3s64",   None,          8, 3, s64);
  (33, U64_3,    "3u64",   None,          8, 3, u64);
  (34, Float3,   "3f",     None,          4, 3, f32);
  (35, Double3,  "3d",     None,          8, 3, f64);
  (36, S8_4,     "4s8",    None,          1, 4, s8);
  (37, U8_4,     "4u8",    None,          1, 4, u8);
  (38, S16_4,    "4s16",   None,          2, 4, s16);
  (39, U16_4,    "4u16",   None,          2, 4, u16);
  (40, S32_4,    "4s32",   Some("vs32"),  4, 4, s32);
  (41, U32_4,    "4u32",   Some("vu32"),  4, 4, u32);
  (42, S64_4,    "4s64",   None,          8, 4, s64);
  (43, U64_4,    "4u64",   None,          8, 4, u64);
  (44, Float4,   "4f",     Some("vf"),    4, 4, f32);
  (45, Double4,  "4d",     None,          8, 4, f64);
  // 46 = Attribute
  // no 47
  (48, Vs8,      "vs8",    None,          1, 16, s8);
  (49, Vu8,      "vu8",    None,          1, 16, u8);
  (50, Vs16,     "vs16",   None,          1, 8, s16);
  (51, Vu16,     "vu16",   None,          1, 8, u16);
  (52, Boolean,  "bool",   Some("b"),     1, 1, bool);
  (53, Boolean2, "2b",     None,          1, 2, bool);
  (54, Boolean3, "3b",     None,          1, 3, bool);
  (55, Boolean4, "4b",     None,          1, 4, bool);
  (56, Vb,       "vb",     None,          1, 16, bool);

  ( 1, NodeStart, "void", None, 0, 0, invalid);
  (46, Attribute, "attr", None, 0, 0, invalid);

  (190, NodeEnd, "nodeEnd", None, 0, 0, invalid);
  (191, FileEnd, "fileEnd", None, 0, 0, invalid);
}
