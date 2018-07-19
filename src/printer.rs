use error::Result;
use node_types::StandardType;
use reader::Reader;

pub struct Printer;

impl Printer {
  pub fn run(input: &[u8]) -> Result<()> {
    let mut reader = Reader::new(input)?;
    let mut nodes = Vec::new();

    while let Ok((node_type, is_array)) = reader.read_node_type() {
      let identifier = match node_type {
        StandardType::NodeEnd |
        StandardType::FileEnd => None,
        _ => Some(reader.read_node_identifier()?),
      };
      nodes.push((node_type, is_array, identifier));

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

    Ok(())
  }
}
