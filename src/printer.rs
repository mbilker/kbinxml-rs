use error::Result;
use node_types::StandardType;
use reader::Reader;

pub struct Printer;

impl Printer {
  pub fn run(input: &[u8]) -> Result<()> {
    let mut reader = Reader::new(input)?;
    let mut nodes = Vec::new();
    let mut definitions = Vec::new();

    while let Ok(def) = reader.read_node_definition() {
      eprintln!("definition: {:?}", def);

      let node_type = def.node_type;
      let key = match def.key() {
        Ok(v) => v,
        Err(e) => {
          error!("error processing key for definition {:?}: {}", def, e);
          None
        },
      };
      nodes.push((node_type, def.is_array, key));
      definitions.push(def);

      if node_type == StandardType::FileEnd {
        break;
      }
    }

    let mut indent = 0;
    for (node_type, is_array, identifier) in nodes {
      eprint!("{:indent$} - {:?} (is_array: {}", "", node_type, is_array, indent = indent);
      if let Some(identifier) = identifier {
        eprint!(", identifier: {}", identifier);
      }
      eprintln!(")");

      match node_type {
        StandardType::Attribute => {},
        StandardType::NodeEnd => indent -= 2,
        _ => indent += 2,
      };
    }

    for def in definitions.into_iter() {
      match def.node_type {
        StandardType::NodeStart |
        StandardType::NodeEnd |
        StandardType::FileEnd => {},
        _ => {
          let node = def.into_node();
          eprintln!("node: {:?}", node);
        },
      }
    }

    Ok(())
  }
}
