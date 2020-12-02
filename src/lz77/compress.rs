use crate::lz77::window_byte_container::WindowByteContainer;
use std::cmp;
use std::convert::TryFrom;
use std::io::BufWriter;
use std::io::Write;

use crate::lz77::nodes::Node;

const SEARCH_WINDOW_SIZE: u16 = 2048;
const PREFIX_WINDOW_SIZE: u16 = 2048;

pub fn build_lz77_node_list<C>(to_compress: &[u8], mut callback: C)
where
    C: FnMut(Node),
{
    let mut byte_ptr = 0;

    loop {
        let c = to_compress[byte_ptr];
        let search_slice_start_index = byte_ptr.saturating_sub(usize::from(SEARCH_WINDOW_SIZE) - 1);
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
pub fn decompress_nodes<W: Write>(nodes: Vec<Node>, writer: &mut W) {
    let mut search_buffer: WindowByteContainer<u8> =
        WindowByteContainer::new(usize::from(SEARCH_WINDOW_SIZE));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_node_list() {
        let bytes = vec![
            b'a', b'b', b'a', b'b', b'c', b'b', b'a', b'b', b'a', b'b', b'a', b'a',
        ];

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
    }

    #[test]
    fn node_offset_cannot_exceed_2047() {
        let mut bytes: Vec<u8> = vec![0; 2060];
        bytes[0] = 0;
        bytes[1] = 1;
        bytes[2048] = 1;
        bytes[2049] = 1;
        bytes[2050] = 0;

        let mut nodes = Vec::new();
        build_lz77_node_list(&bytes, |node| nodes.push(node));

        assert!(nodes.iter().all(|e| e.offset < 2048))
    }

    #[test]
    fn node_list_test_no_trailing_chars() {
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
}
