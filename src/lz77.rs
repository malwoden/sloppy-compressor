use bitvec::prelude::*;
use serde::{Deserialize, Serialize};
use std::{cmp, fs::File};
use std::{
    collections::VecDeque,
    io::{self, prelude::*, BufWriter},
};
use std::{convert::TryFrom, mem::size_of};

use super::compression;
use crate::compression::Algorithm;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

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
    fn append_end_marker_adds_byte_padding() {
        let mut vec = bitvec![Msb0, u8;];
        append_end_marker(&mut vec);
        assert_eq!(
            bitvec![1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,],
            vec
        );

        vec = bitvec![Msb0, u8;1, 1, 1];
        append_end_marker(&mut vec);
        assert_eq!(
            bitvec![1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,],
            vec
        );
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

    #[test]
    fn writes_compressed_file() {
        // a bad test - but will help catch some clear errors whilst under dev
        let mut input_file = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut output_file = input_file.clone();
        input_file.push("src/bitvec_issues.rs");
        output_file.push("src/bitvec_issues.rs.testout");

        let f = File::open(input_file).unwrap();
        let c = Lz77Compression {};
        c.compress(f, output_file.to_str().unwrap()).unwrap();
    }

    #[test]
    fn converts_slice_to_byte() {
        let slice = BitSlice::<Msb0, u8>::from_element(&b'w');
        let byte = slice_to_byte(slice);
        assert_eq!(byte, b'w');
    }

    #[test]
    fn deserialises_length() {
        assert_eq!((2, 2), deserialise_length(bits![Msb0, u8; 0,0,0,0]));
        assert_eq!((3, 2), deserialise_length(bits![Msb0, u8; 0,1,0,0]));
        assert_eq!((4, 2), deserialise_length(bits![Msb0, u8; 1,0,0,0]));
        assert_eq!((5, 4), deserialise_length(bits![Msb0, u8; 1,1,0,0]));
        assert_eq!((6, 4), deserialise_length(bits![Msb0, u8; 1,1,0,1]));
        assert_eq!((7, 4), deserialise_length(bits![Msb0, u8; 1,1,1,0]));
        assert_eq!((8, 8), deserialise_length(bits![Msb0, u8; 1,1,1,1,0,0,0,0]));
        assert_eq!((9, 8), deserialise_length(bits![Msb0, u8; 1,1,1,1,0,0,0,1]));
        assert_eq!(
            (23, 12),
            deserialise_length(bits![Msb0, u8; 1,1,1,1,1,1,1,1,0,0,0,0])
        );
        assert_eq!(
            (37, 12),
            deserialise_length(bits![Msb0, u8; 1,1,1,1,1,1,1,1,1,1,1,0])
        );
        assert_eq!(
            (38, 16),
            deserialise_length(bits![Msb0, u8; 1,1,1,1,1,1,1,1,1,1,1,1,0,0,0,0])
        );

        let mut max_val = bitvec![Msb0, u8;];
        let four_bit_blocks_for_max_size = ((2047 + 7) / 15) + 1; // +1 for final 4 bits;
        assert_eq!(137, four_bit_blocks_for_max_size);

        max_val.resize(137 * 4, true);
        max_val.set(544, true);
        max_val.set(545, true);
        max_val.set(546, true);
        max_val.set(547, false);
        assert_eq!((2047, 548), deserialise_length(&max_val));
    }

    #[test]
    fn length_encode_decode_in_harmony() {
        assert_eq!((2, 2), deserialise_length(&serialise_length(2)));
        assert_eq!((7, 4), deserialise_length(&serialise_length(7)));
        assert_eq!((8, 8), deserialise_length(&serialise_length(8)));
        assert_eq!((23, 12), deserialise_length(&serialise_length(23)));
        assert_eq!((77, 24), deserialise_length(&serialise_length(77)));
        assert_eq!((1024, 276), deserialise_length(&serialise_length(1024)));
    }

    #[test]
    fn serialise_and_deserialise_nodes() {
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
        let mut serialised = serailise_nodes(&nodes);
        append_end_marker(&mut serialised);
        let deserialised = deserialise_nodes(serialised.into());
        assert_eq!(nodes, deserialised);
    }
}

#[derive(PartialEq, Debug, Serialize, Deserialize)]
struct Node {
    offset: u16,
    length: u16,
    char: u8,
}

const SEARCH_WINDOW_SIZE: u16 = 2048;
const PREFIX_WINDOW_SIZE: u16 = 2048;

const U16_BIT_SIZE: usize = size_of::<u16>() * 8;

#[derive(Serialize, Deserialize, Debug)]
struct Compressed {
    search_window_size: u16,
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

        let mut encoded_nodes = serailise_nodes(&nodes);
        append_end_marker(&mut encoded_nodes);
        let bv: Vec<u8> = encoded_nodes.into();

        compression::write_to_new_file(&bv, output_file_path)
    }

    fn decompress(&self, mut compressed_file: File, output_file_path: &str) -> io::Result<()> {
        let mut file_bytes: Vec<u8> = vec![];
        compressed_file.read_to_end(&mut file_bytes)?;

        let nodes = deserialise_nodes(file_bytes);

        // write nodes?

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
                for b in x[U16_BIT_SIZE - 7..].iter() {
                    vec.push(*b);
                }
            } else {
                vec.push(false);
                for b in x[U16_BIT_SIZE - 11..].iter() {
                    vec.push(*b);
                }
            }

            let length_encoded = serialise_length(node.length);
            append_bitvecs(&mut vec, &length_encoded);
        } else {
            // literal byte - push '0' followed by 8 bits for the byte val
            vec.push(false);
        }
        let literal = BitSlice::<Msb0, u8>::from_element(&node.char);
        append_bitvecs(&mut vec, &literal.to_bitvec());
    }

    vec
}

fn serialise_length(length: u16) -> BitVec<Msb0, u8> {
    return match length {
        1 => panic!("Nodes should not have a size of 1"),
        2 => bitvec![Msb0, u8;0, 0],
        3 => bitvec![Msb0, u8;0, 1],
        4 => bitvec![Msb0, u8;1, 0],
        5 => bitvec![Msb0, u8;1, 1, 0, 0],
        6 => bitvec![Msb0, u8;1, 1, 0, 1],
        7 => bitvec![Msb0, u8;1, 1, 1, 0],
        _ => {
            let mut encoded = bitvec![Msb0, u8;];
            let padding_one_blocks = (length + 7) / 15;

            for _ in 0..padding_one_blocks {
                let mut padding_block = bitvec![1, 1, 1, 1];
                encoded.append(&mut padding_block);
            }

            let adjusted = length - (padding_one_blocks * 15 - 7);
            let adjusted = u8::try_from(adjusted).unwrap();
            let bits = adjusted.view_bits::<Msb0>();
            encoded.extend_from_bitslice(&bits[4..]);
            encoded
        }
    };
}

fn deserialise_nodes(file_bytes: Vec<u8>) -> Vec<Node> {
    let end_of_stream_marker = bits![Msb0, u8; 1, 1, 0, 0, 0, 0, 0, 0, 0];

    let mut nodes: Vec<Node> = vec![];
    let bit_view = file_bytes.view_bits::<Msb0>();

    let mut bitstream_offset = 0;
    while bitstream_offset < bit_view.len() {
        let is_reference_node = bit_view[bitstream_offset];
        bitstream_offset += 1;

        if !is_reference_node {
            // next 8 bits will be a literal byte node
            let byte_literal = &bit_view[bitstream_offset..bitstream_offset + 8];
            nodes.push(Node {
                length: 0,
                offset: 0,
                char: slice_to_byte(&byte_literal),
            });
            bitstream_offset += 8;
        } else {
            // flag 1: this is a node reference
            let offset_sub_128 = bit_view[bitstream_offset];
            bitstream_offset += 1;

            let offset: u16;
            if offset_sub_128 {
                // 7 bits for the offset size
                offset = slice_to_offset(&bit_view[bitstream_offset..bitstream_offset + 7]);
                bitstream_offset += 7;
            } else {
                // 11 bits for the offset
                offset = slice_to_offset(&bit_view[bitstream_offset..bitstream_offset + 11]);
                bitstream_offset += 11;
            }

            let (length, bits_read) = deserialise_length(&bit_view[bitstream_offset..]);
            bitstream_offset += usize::from(bits_read);

            // next 8 bits will be a literal byte node
            let byte_literal = &bit_view[bitstream_offset..bitstream_offset + 8];
            nodes.push(Node {
                length,
                offset,
                char: slice_to_byte(&byte_literal),
            });
            bitstream_offset += 8;
        }

        if bit_view[bitstream_offset..bitstream_offset + 9] == end_of_stream_marker {
            break;
        }
    }
    nodes
}

/// Extract the length from the encoded bit array
///
/// Expectation is the slice starts at the first bit of the encoded length, to the end of the stream.
///
/// Returns a tuple in the form (length, num bits consumed)
fn deserialise_length(slice: &BitSlice<Msb0, u8>) -> (u16, u16) {
    let two_bit_size = &slice[0..2];
    if two_bit_size != bits![Msb0, u8; 1, 1] {
        if two_bit_size == bits![Msb0, u8; 0, 0] {
            return (2, 2);
        }

        if two_bit_size == bits![Msb0, u8; 0, 1] {
            return (3, 2);
        }

        return (4, 2); // 1, 1
    }

    let four_bit_size = &slice[0..4];
    if four_bit_size != bits![Msb0, u8; 1, 1, 1, 1] {
        if four_bit_size == bits![Msb0, u8; 1, 1, 0, 0] {
            return (5, 4);
        }

        if four_bit_size == bits![Msb0, u8; 1, 1, 0, 1] {
            return (6, 4);
        }

        return (7, 4);
    }

    let four_bits_all_set = bits![Msb0, u8; 1,1,1,1];
    let mut four_bit_block_count = 0;
    loop {
        // iterate through the bit slice, find the first non 1,1,1,1 block then reverse the encoding.
        let block_index = four_bit_block_count * 4;
        let block_bits = &slice[block_index..block_index + 4];
        if block_bits == four_bits_all_set {
            four_bit_block_count += 1;
        } else {
            // non 1,1,1,1 sequence found - read the next for bits then invert the enoding formula:
            // (1111 repeated N times) xxxx, where  is integer result of (length + 7) / 15, and xxxx is length - (N*15 − 7)
            let trailing_bit_value = slice_to_offset(block_bits);
            let length = (four_bit_block_count * 15 - 7) + trailing_bit_value as usize;

            // +4 to account for the non: 1,1,1,1 block at the end of the encoded length
            let next_read_offset = u16::try_from(block_index + 4).unwrap();
            return (u16::try_from(length).unwrap(), next_read_offset);
        }
    }
}

fn slice_to_byte<T>(slice: &BitSlice<Msb0, T>) -> u8
where
    T: BitStore,
{
    let mut as_byte: u8 = 0;
    for (i, flag) in slice.iter().rev().enumerate() {
        if *flag {
            as_byte = as_byte | (1 << i);
        }
    }

    as_byte
}

fn slice_to_offset<T>(slice: &BitSlice<Msb0, T>) -> u16
where
    T: BitStore,
{
    let mut as_byte: u16 = 0;
    for (i, flag) in slice.iter().rev().enumerate() {
        if *flag {
            as_byte = as_byte | (1 << i);
        }
    }

    as_byte
}

/// Adds the end-of-stream bit sequence and pads the vector to a whole byte
fn append_end_marker<O, T>(encoding: &mut BitVec<O, T>)
where
    O: BitOrder,
    T: BitStore,
{
    encoding.append(&mut bitvec![Msb0, u8; 1, 1, 0, 0, 0, 0, 0, 0, 0]);
    let trailing_bits = encoding.len() % 8;
    if trailing_bits > 0 {
        for _ in 0..(8 - trailing_bits) {
            encoding.push(false);
        }
    }
}

fn append_bitvecs<O, T>(original: &mut BitVec<O, T>, to_add: &BitVec<O, T>)
where
    O: BitOrder,
    T: BitStore,
{
    for b in to_add.iter() {
        original.push(*b);
    }
    // dont use append/extend as they vastly slower: https://github.com/myrrlyn/bitvec/issues/94
}

fn build_lz77_node_list<C>(to_compress: &[u8], mut callback: C)
where
    C: FnMut(Node),
{
    let mut byte_ptr = 0;

    loop {
        let c = to_compress[byte_ptr];
        let search_slice_start_index = byte_ptr.saturating_sub(usize::from(SEARCH_WINDOW_SIZE));
        let search_slice_end_index = byte_ptr;

        let prefix_slice_start_index = byte_ptr + 1;
        let prefix_slice_end_index = cmp::min(
            byte_ptr.saturating_add(usize::from(PREFIX_WINDOW_SIZE)),
            to_compress.len(),
        );

        let search_slice = &to_compress[search_slice_start_index..search_slice_end_index];
        let prefix_slice = &to_compress[prefix_slice_start_index..prefix_slice_end_index];

        let node = calculate_node(c, search_slice, prefix_slice);
        byte_ptr += 1 + usize::from(node.length); // advance the byte pointer 1 past the position of char literal in the node

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
        offset: u16::try_from(offset).unwrap(),
        length: u16::try_from(length).unwrap(),
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
fn decompress_nodes<W: Write>(nodes: Vec<Node>, writer: &mut W, search_window_size: u16) {
    let mut search_buffer: WindowByteContainer<u8> =
        WindowByteContainer::new(usize::from(search_window_size));
    let mut buffered_writer = BufWriter::new(writer);

    for node in nodes {
        let mut bytes_to_write = Vec::new();
        if node.length > 0 {
            // copy from the search buffer
            let search_start_index = search_buffer.vec.len() - usize::from(node.offset);
            let search_stop_index = search_start_index + usize::from(node.length);
            let b = search_buffer
                .vec
                .range(search_start_index..search_stop_index);
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
