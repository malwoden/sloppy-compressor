use std::fs::File;
use std::io::{self, prelude::*};

use crate::compression;

mod compress;
mod nodes;
mod serialisation;
mod window_byte_container;

pub struct Lz77Compression {}

impl compression::Algorithm for Lz77Compression {
    fn compress(&self, mut file: File, output_file_path: &str) -> io::Result<()> {
        let mut file_bytes = Vec::new();
        file.read_to_end(&mut file_bytes)
            .expect("Error on file read");

        self.compress_bytes(&file_bytes, output_file_path)
    }

    fn decompress(&self, mut compressed_file: File, output_file_path: &str) -> io::Result<()> {
        let mut file_bytes: Vec<u8> = vec![];
        compressed_file.read_to_end(&mut file_bytes)?;

        let nodes = serialisation::deserialise_nodes(&file_bytes);

        let mut file = File::create(output_file_path)?;
        compress::decompress_nodes(nodes, &mut file);
        Ok(())
    }
}

impl Lz77Compression {
    pub fn compress_bytes(&self, file_bytes: &[u8], output_file_path: &str) -> io::Result<()> {
        let mut nodes = Vec::new();
        compress::build_lz77_node_list(file_bytes, |node| nodes.push(node));

        let mut encoded_nodes = serialisation::serailise_nodes(&nodes);
        serialisation::append_end_marker(&mut encoded_nodes);
        let bv: Vec<u8> = encoded_nodes.into();

        compression::write_to_new_file(&bv, output_file_path)
    }

    pub fn decompress_bytes(
        &self,
        compressed_bytes: &Vec<u8>,
        output_file_path: &str,
    ) -> io::Result<()> {
        let nodes = serialisation::deserialise_nodes(compressed_bytes);

        let mut file = File::create(output_file_path)?;
        compress::decompress_nodes(nodes, &mut file);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{compression::Algorithm, lz77::nodes::NodeType};
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn decompresses_to_original_bytes() {
        let bytes = vec![
            b'a', b'b', b'a', b'b', b'c', b'b', b'a', b'b', b'a', b'b', b'a', b'a',
        ];

        // (0,0,a), (0,0,b), (2,2,c), (4,3,a), (2,2,a)
        let expected: Vec<NodeType> = vec![
            NodeType::ByteLiteral { lit: b'a' },
            NodeType::ByteLiteral { lit: b'b' },
            NodeType::Reference {
                length: 2,
                offset: 2,
            },
            NodeType::ByteLiteral { lit: b'c' },
            NodeType::Reference {
                length: 3,
                offset: 4,
            },
            NodeType::ByteLiteral { lit: b'a' },
            NodeType::Reference {
                length: 2,
                offset: 2,
            },
            NodeType::ByteLiteral { lit: b'a' },
        ];
        let mut nodes = Vec::new();
        compress::build_lz77_node_list(&bytes, |node| nodes.push(node));
        assert_eq!(expected, nodes);

        let mut write_vec: Vec<u8> = Vec::new();
        compress::decompress_nodes(nodes, &mut write_vec);
        assert_eq!(write_vec, bytes);
    }
}
