# kbinxml-rs

An encoder/decoder for Konami's binary XML format, used in many of their games.

Requires Rust 1.34 or newer!

## Setup

- Setup Rust through `rustup` or your own preferred method of acquiring Rust
- For using `kbinxml-rs` as a library, add it as a dependency in your `Cargo.toml` file
  - For example, if you want the base library and psmap_derive:
  - `cargo add kbinxml psmap psmap_derive --git https://github.com/mbilker/kbinxml-rs.git`
- For using `kbinxml-rs` as a standalone application, install `kbinxml-rs` using `cargo install kbinxml --features=build_binary` (Note: This will not work at the moment as `kbinxml-rs` has not yet been published to [crates.io](https://crates.io))

## Code Examples

### Deserialisation

#### From text
```rust
use kbinxml;

fn main() {
    let input = b"
    <?xml version='1.0'?>
    <test>
        <entry __type=\"str\" some_attr=\"an attribute\">Hello, world!</entry>
    </test>
    ";

    let (nodes, encoding) = kbinxml::from_text_xml(input).unwrap();

    println!("encoding: {}, data: {:?}", encoding, nodes);
}
```

This prints (prettified for the README):
```rust
encoding: UTF-8, data: NodeCollection {
    base: NodeDefinition {
        encoding: UTF_8,
        node_type: NodeStart,
        is_array: false,
        data: Some {
            key: Uncompressed { "test" },
            value_data: b""
        }
    },
    attributes: [],
    children: [
        NodeCollection {
            base: NodeDefinition {
                encoding: UTF_8,
                node_type: String,
                is_array: false,
                data: Some {
                    key: Uncompressed { "entry" },
                    value_data: b"Hello, world!\0"
                }
            },
            attributes: [
                NodeDefinition {
                    encoding: UTF_8,
                    node_type: Attribute,
                    is_array: false,
                    data: Some {
                        key: Uncompressed { "some_attr" },
                        value_data: b"an attribute\0"
                    }
                }
            ],
            children: []
        }
    ]
}
```

#### From a file, with binary/text auto-detect
```rust
use std::fs;
use kbinxml;

fn main() {
    let input = fs::read("testcases_out.kbin").unwrap();
    let (nodes, encoding) = kbinxml::from_binary(input.into()).unwrap();

    println!("encoding: {}, data: {:?}", encoding, nodes);
}
```

#### Using psmap_derive

##### On a single set of nodes:  
```rust
use kbinxml;
use psmap;
use psmap_derive::*;

#[derive(Debug)]
struct Entry {
    hello: String,
    some_attr: String,
    sub_entry_value: Option<u8>,
    sub_entry_attr: u8,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = b"
    <?xml version='1.0'?>
    <test>
        <entry __type=\"str\" some_attr=\"an attribute\">
            Hello, world!
            <sub_entry __type=\"u8\" another_attr=\"10\">64</sub_entry>
        </entry>
    </test>
    ";

    let (nodes, _encoding) = kbinxml::from_text_xml(input).unwrap();

    // the container NodeCollection must be turned into a single node for psmap
    let node = nodes.as_node().unwrap();

    // psmap uses `?` internally, so the calling function must return a Result.
    // If you're making a larger project, using `anyhow::Result` is recommended.
    let value = psmap! {
        output: Entry,
        inputs: [
            // the top-level node "test" is inaccessible, only the child nodes
            // and attributes are usable inside psmap
            node: {
                "entry" => {
                    attributes: {
                        "some_attr" => some_attr,
                    },
                    value => hello,

                    "sub_entry" => {
                        attributes: {
                            // attributes can be parsed with `as`
                            "another_attr" => sub_entry_attr as u8,
                        },
                        value => sub_entry_value,
                        // A value can be made optional using this keyword
                        optional,
                    },
                }
            },
        ],
    };

    println!("{:?}", value);

    Ok(())
}
```

This code prints:
```rust
Entry {
    hello: "Hello, world!",
    some_attr: "an attribute",
    sub_entry_value: Some(64),
    sub_entry_attr: 10
}
```

##### On a list of identical nodes (such as a music db):
```rust
use kbinxml;
use psmap;
use psmap_derive::*;

#[derive(Debug)]
struct Entry {
    id: u8,
    name: String,
    color: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = b"
    <?xml version='1.0'?>
    <test>
        <!-- Leaf nodes with no type are 'str' by default -->
        <entry id=\"5\"><name>Banana</name><color>Yellow</color></entry>
        <entry id=\"8\"><name>Apple</name><color>Red</color></entry>
        <entry id=\"1\"><name>Peach</name><color>Pink</color></entry>
        <entry id=\"2\"><name>Mulberry</name><color>Violet</color></entry>
        <entry id=\"7\"><name>Rockmelon</name><color>Orange</color></entry>
        <entry id=\"4\"><name>Disguised Banana</name><color>Invisible</color></entry>
    </test>
    ";

    let (nodes, _encoding) = kbinxml::from_text_xml(input).unwrap();

    let entries: Result<Vec<Entry>, _> = nodes.children().iter()
        .map(|node_collection| -> Result<Entry, Box<dyn std::error::Error>> {
            let node = node_collection.as_node()?;
            // because we cannot access the top level node "entry", we get its
            // attribute first
            let id = node
                .attributes()
                .get("id").expect("id not present")
                .parse::<u8>()?;

            // we then provide the `id` to `psmap` using "include"
            Ok(psmap! {
                output: Entry,
                include: [id],
                inputs: [
                    node: {
                        "name" => name,
                        "color" => color,
                    },
                ],
            })
        })
        .collect();

    println!("{:?}", entries);
    
    Ok(())
}
```

This code prints (prettified for the README):
```rust
Ok([
    Entry { id: 5, name: "Banana", color: "Yellow" },
    Entry { id: 8, name: "Apple", color: "Red" },
    Entry { id: 1, name: "Peach", color: "Pink" },
    Entry { id: 2, name: "Mulberry", color: "Violet" },
    Entry { id: 7, name: "Rockmelon", color: "Orange" },
    Entry { id: 4, name: "Disguised Banana", color: "Invisible" }
])
```

### Serialisation

#### To text
```rust
let text = kbinxml::to_text_xml(&nodes).unwrap();
// if you encoded in UTF8 and want a str:
let text = std::str::from_utf8(&text).unwrap();
```

#### To bytes
```rust
let bytes = kbinxml::to_binary(&nodes).unwrap();
```

#### To bytes, with encoding options
```rust
let options = kbinxml::Options::builder()
    .compression(kbinxml::CompressionType::Compressed)
    .encoding(kbinxml::EncodingType::SHIFT_JIS)
    .build();
let bytes = kbinxml::to_binary_with_options(options, &nodes).unwrap();
```

#### From a struct
There is currently no way to seralise structs directly as with `psmap`, they
must be converted into nodes manually.
