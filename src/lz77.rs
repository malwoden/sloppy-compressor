use bitvec::prelude::*;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::{cmp, fs::File};
use std::{
    collections::VecDeque,
    io::{self, prelude::*, BufReader, BufWriter},
};

use super::compression;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_lz77_node_list_test() {
        let bytes = vec![
            b'a', b'b', b'a', b'b', b'c', b'b', b'a', b'b', b'a', b'b', b'a', b'a',
        ]; // D:

        // (0,0,a), (0,0,b), (2,2,c), (4,3,a), (2,2,a)
        let expected: Vec<Node> = vec![
            Node {
                offset: 0,
                length: 0,
                char: b'a',
            },
            Node {
                offset: 0,
                length: 0,
                char: b'b',
            },
            Node {
                offset: 2,
                length: 2,
                char: b'c',
            },
            Node {
                offset: 4,
                length: 3,
                char: b'a',
            },
            Node {
                offset: 2,
                length: 2,
                char: b'a',
            },
        ];
        let mut nodes = Vec::new();
        build_lz77_node_list(&bytes, |node| nodes.push(node));
        assert_eq!(expected, nodes);

        let mut write_vec: Vec<u8> = Vec::new();
        decompress_nodes(nodes, &mut write_vec, 4096);
        assert_eq!(write_vec, bytes);
    }

    #[test]
    fn build_lz77_node_list_test_no_trailing_chars() {
        let bytes = vec![b'a', b'b', b'a', b'b', b'b']; // D:

        // (0,0,a), (0,0,b), (2,2,b)
        let expected: Vec<Node> = vec![
            Node {
                offset: 0,
                length: 0,
                char: b'a',
            },
            Node {
                offset: 0,
                length: 0,
                char: b'b',
            },
            Node {
                offset: 2,
                length: 2,
                char: b'b',
            },
        ];
        let mut nodes = Vec::new();
        build_lz77_node_list(&bytes, |node| nodes.push(node));
        assert_eq!(expected, nodes);
    }

    #[test]
    fn serailise_nodes_handles_literals_and_refs() {
        let nodes: Vec<Node> = vec![
            Node {
                offset: 0,
                length: 0,
                char: b'a',
            },
            Node {
                offset: 0,
                length: 0,
                char: b'b',
            },
            Node {
                offset: 2,
                length: 2,
                char: b'b',
            },
        ];
        // 0 01100001 0 01100010 11 0000010 00 01100010
        // 0 = char lit
        // next 8 bits are the 'a' char
        // repeat for 'b' char
        // 11 - a reference node, offset <128
        // 0000010 - 7 bit offset value: 2
        // 00 - length value, in this case '2'
        // last 8 bits are the char literal 'b'
        let expected = bitvec![
            0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 0, 1, 0, 1, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0,
            0, 1, 1, 0, 0, 0, 1, 0,
        ];
        assert_eq!(expected, serailise_nodes(&nodes));
    }

    #[test]
    fn serailise_nodes_large_lengths() {
        let nodes = vec![Node {
            offset: 17,
            length: 8,
            char: b'a',
        }];
        assert_eq!(
            bitvec![1, 1, 0, 0, 1, 0, 0, 0, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1],
            serailise_nodes(&nodes)
        );

        let nodes = vec![Node {
            offset: 17,
            length: 24,
            char: b'a',
        }];
        assert_eq!(
            bitvec![
                1, 1, 0, 0, 1, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 1, 0, 1, 1, 0, 0, 0, 0,
                1
            ],
            serailise_nodes(&nodes)
        );
    }

    #[test]
    fn serailise_nodes_size_experiment() {
        // Q: What is most efficient: 3 raw bytes or 1 raw byte and a 2 byte-len node ref?

        let three_raw_bytes = vec![
            Node {
                offset: 0,
                length: 0,
                char: b'a',
            },
            Node {
                offset: 0,
                length: 0,
                char: b'a',
            },
            Node {
                offset: 0,
                length: 0,
                char: b'a',
            },
        ];
        assert_eq!(
            bitvec![
                0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 0, 0, 1,
            ],
            serailise_nodes(&three_raw_bytes)
        );

        let two_length_node_ref = vec![Node {
            offset: 2,
            length: 2,
            char: b'a',
        }];
        assert_eq!(
            bitvec![1, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1,],
            serailise_nodes(&two_length_node_ref)
        );

        // A: the reference is always smaller, even for larger offset values that require extra bits
    }

    #[test]
    fn windowing_buffer() {
        let mut search_buffer: WindowByteContainer<u8> = WindowByteContainer::new(4);
        search_buffer.push(b'a');
        search_buffer.push_all(&[b'b', b'c', b'd', b'e']);

        assert_eq!(search_buffer.vec, vec![b'b', b'c', b'd', b'e']);
        search_buffer.push(b'z');
        assert_eq!(search_buffer.vec, vec![b'c', b'd', b'e', b'z']);
    }
}

#[derive(PartialEq, Debug, Serialize, Deserialize)]
struct Node {
    offset: usize,
    length: usize,
    char: u8,
}

const SEARCH_WINDOW_SIZE: usize = 2048;
const PREFIX_WINDOW_SIZE: usize = 2048;

#[derive(Serialize, Deserialize, Debug)]
struct Compressed {
    search_window_size: usize,
    nodes: Vec<Node>,
}

pub struct Lz77Compression {}

impl compression::Algorithm for Lz77Compression {
    fn compress(&self, mut file: File, output_file_path: &str) -> io::Result<()> {
        let mut file_bytes = Vec::new();
        file.read_to_end(&mut file_bytes)
            .expect("Error on file read");

        let mut nodes = Vec::new();
        build_lz77_node_list(&file_bytes, |node| nodes.push(node));
        println!("Compression Nodes: {:?}", nodes.len());
        println!("Last Nodes: {:?}", &nodes[nodes.len() - 20..]);

        println!("as bits: {:?}", serailise_nodes(&nodes).len() / 8);

        compression::write_compressed(
            &Compressed {
                search_window_size: SEARCH_WINDOW_SIZE,
                nodes,
            },
            output_file_path,
        )
    }

    fn decompress(&self, compressed_file: File, output_file_path: &str) -> io::Result<()> {
        let compressed: Compressed =
            bincode::deserialize_from(BufReader::new(compressed_file)).unwrap();
        let mut file = File::create(output_file_path)?;
        decompress_nodes(compressed.nodes, &mut file, compressed.search_window_size);
        Ok(())
    }
}

fn serailise_nodes(nodes: &Vec<Node>) -> BitVec<Msb0, u8> {
    let mut vec = bitvec![Msb0, u8;];
    // Don't reserve here as a bug in bit-vec results in slower extend/append ops.

    for node in nodes {
        if node.length > 0 {
            // offset / length reference
            vec.push(true);
            let x = node.offset.view_bits::<Msb0>();
            if node.offset < 128 {
                vec.push(true);
                for b in x[64 - 7..].iter() {
                    vec.push(*b);
                }
            } else {
                vec.push(false);
                for b in x[64 - 11..].iter() {
                    vec.push(*b);
                }
            }

            let length_encoded = match node.length {
                1 => panic!("Nodes should not have a size of 1"),
                2 => bitvec![Msb0, u8;0, 0],
                3 => bitvec![Msb0, u8;0, 1],
                4 => bitvec![Msb0, u8;1, 0],
                5 => bitvec![Msb0, u8;1, 1, 0, 0],
                6 => bitvec![Msb0, u8;1, 1, 0, 1],
                7 => bitvec![Msb0, u8;1, 1, 1, 0],
                _ => {
                    let mut encoded = bitvec![Msb0, u8;];
                    let padding_one_blocks = (node.length + 7) / 15;

                    for _ in 0..padding_one_blocks {
                        let mut padding_block = bitvec![1, 1, 1, 1];
                        encoded.append(&mut padding_block);
                    }

                    let adjusted = node.length - (padding_one_blocks * 15 - 7);
                    let adjusted = u8::try_from(adjusted).unwrap();
                    let bits = adjusted.view_bits::<Msb0>();
                    encoded.extend_from_bitslice(&bits[4..]);
                    encoded
                }
            };

            append_bitvecs(&mut vec, &length_encoded);
        } else {
            // literal byte - push '0' followed by 8 bits for the byte val
            vec.push(false);
        }
        let literal = BitSlice::<Msb0, u8>::from_element(&node.char);
        append_bitvecs(&mut vec, &literal.to_bitvec());
    }

    // TODO: add end marker and padding
    // vec.append(&mut bitvec![1, 1, 0, 0, 0, 0, 0, 0, 0]);

    vec
}

fn append_bitvecs(original: &mut BitVec<Msb0, u8>, to_add: &BitVec<Msb0, u8>) {
    for b in to_add.iter() {
        original.push(*b);
    }
    // dont use append/extend as they are slow
}

fn build_lz77_node_list<C>(to_compress: &[u8], mut callback: C)
where
    C: FnMut(Node),
{
    let mut byte_ptr = 0;

    loop {
        let c = to_compress[byte_ptr];
        let search_slice_start_index = byte_ptr.saturating_sub(SEARCH_WINDOW_SIZE);
        let search_slice_end_index = byte_ptr;

        let prefix_slice_start_index = byte_ptr + 1;
        let prefix_slice_end_index = cmp::min(
            byte_ptr.saturating_add(PREFIX_WINDOW_SIZE),
            to_compress.len(),
        );

        let search_slice = &to_compress[search_slice_start_index..search_slice_end_index];
        let prefix_slice = &to_compress[prefix_slice_start_index..prefix_slice_end_index];

        let node = calculate_node(c, search_slice, prefix_slice);
        byte_ptr += 1 + node.length; // advance the byte pointer 1 past the position of char literal in the node

        callback(node);

        if byte_ptr > to_compress.len() - 1 {
            break;
        }
    }
}

fn calculate_node(
    first_uncompressed_byte: u8,
    compressed_bytes: &[u8],
    bytes_to_compressed: &[u8],
) -> Node {
    let mut offset = 0;
    let mut length = 0;

    // find a byte sequence in the previously compressed bytes.
    // reverse iterate from the end of the compressed bytes, find a matching byte, then
    // moving forward through the compressed bytes and the bytes to be compressed, find the
    // longest matching sequence.
    if compressed_bytes.len() > 0 {
        let start = compressed_bytes.len() - 1;
        let mut i = start;

        loop {
            if first_uncompressed_byte == compressed_bytes[i] {
                let series_match =
                    find_length_of_series_match(&compressed_bytes[i + 1..], bytes_to_compressed);
                if series_match > length {
                    offset = compressed_bytes.len() - i;
                    length = series_match + 1; // + 1 to include the 'first_uncompressed_byte' char
                }
            }

            if i == 0 {
                break;
            }
            i -= 1;
        }
    }

    let next_char;
    if length == 0 {
        // offset and length are 0 - this is a char literal node
        next_char = first_uncompressed_byte;
    } else {
        if length > bytes_to_compressed.len() {
            // we always need to have a char in the Node, but if we find a match up to the end of the
            // right slice, we have no 'next' char to set. So we shorten the matched pattern by 1 char
            // and set that final char as the next char for this node
            length -= 1;
        }

        next_char = bytes_to_compressed[length - 1]
    }

    Node {
        offset,
        length,
        char: next_char,
    }
}

fn find_length_of_series_match(left: &[u8], right: &[u8]) -> usize {
    let max_count = cmp::min(left.len(), right.len());
    for i in 0..max_count {
        if left[i] != right[i] {
            return i;
        }
    }
    max_count
}

// need to keep the search window in memory, which means the length of it needs to be serialised.
fn decompress_nodes<W: Write>(nodes: Vec<Node>, writer: &mut W, search_window_size: usize) {
    let mut search_buffer: WindowByteContainer<u8> = WindowByteContainer::new(search_window_size);
    let mut buffered_writer = BufWriter::new(writer);

    for node in nodes {
        let mut bytes_to_write = Vec::new();
        if node.length > 0 {
            // copy from the search buffer
            let search_start_index = search_buffer.vec.len() - node.offset;
            let b = search_buffer
                .vec
                .range(search_start_index..search_start_index + node.length);
            bytes_to_write.extend(b);
        }
        bytes_to_write.push(node.char);
        buffered_writer
            .write_all(&bytes_to_write)
            .expect("Error during decompression");

        search_buffer.push_all(&bytes_to_write);
    }
}

/// A fixed sized container that pops old elements as new ones arrive
#[derive(PartialEq, Debug)]
struct WindowByteContainer<T> {
    pub vec: VecDeque<T>,
    limit: usize,
}

impl<T: std::marker::Copy> WindowByteContainer<T> {
    fn new(limit: usize) -> WindowByteContainer<T> {
        WindowByteContainer {
            vec: VecDeque::with_capacity(limit),
            limit,
        }
    }

    fn push(&mut self, element: T) {
        if self.vec.len() == self.limit {
            self.vec.pop_front();
        }
        self.vec.push_back(element);
    }

    fn push_all(&mut self, elements: &[T]) {
        while self.vec.len() + elements.len() > self.limit {
            self.vec.pop_front();
        }
        for e in elements {
            self.vec.push_back(*e);
        }
    }
}
