use crate::lz77::window_byte_container::ByteBuffer;
use std::cmp;
use std::convert::TryFrom;
use std::io::BufWriter;
use std::io::Write;

use crate::lz77::nodes::NodeType;
use crate::lz77::window_byte_container::ByteWindow;

const SEARCH_WINDOW_SIZE: u16 = 2047;
const PREFIX_WINDOW_SIZE: u16 = 2048;

pub fn build_lz77_node_list<C>(to_compress: &[u8], mut callback: C)
where
    C: FnMut(NodeType),
{
    let mut byte_ptr = 0;

    let mut search_window =
        ByteWindow::with_max_window_size(to_compress, usize::from(SEARCH_WINDOW_SIZE));
    let mut prefix_window =
        ByteWindow::with_max_window_size(to_compress, usize::from(PREFIX_WINDOW_SIZE));

    loop {
        let c = to_compress[byte_ptr];
        let search_slice = search_window.advance_to_pointer(byte_ptr).window;
        let prefix_slice = prefix_window
            .advance_to_pointer(byte_ptr + usize::from(PREFIX_WINDOW_SIZE) + 1)
            .window;

        match calculate_reference_node(c, search_slice, prefix_slice) {
            Some(NodeType::Reference { offset, length }) => {
                byte_ptr += usize::from(length);
                callback(NodeType::Reference { offset, length });
            }
            Some(_) => panic!("Only Refernce nodes should be returned"),
            None => {
                callback(NodeType::ByteLiteral {
                    lit: c,
                });
                byte_ptr += 1;
            }
        }

        if byte_ptr > to_compress.len() - 1 {
            break;
        }
    }
}

fn calculate_reference_node(
    first_uncompressed_byte: u8,
    compressed_bytes: &[u8],
    bytes_to_compressed: &[u8],
) -> Option<NodeType> {
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

    if length == 0 {
        return Option::None::<NodeType>;
    }

    Option::Some(NodeType::Reference {
        offset: u16::try_from(offset).unwrap(),
        length: u16::try_from(length).unwrap(),
    })
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
pub fn decompress_nodes<W: Write>(nodes: Vec<NodeType>, writer: &mut W) {
    let mut search_buffer: ByteBuffer<u8> = ByteBuffer::new(usize::from(SEARCH_WINDOW_SIZE));
    let mut buffered_writer = BufWriter::new(writer);

    for node in nodes {
        let mut bytes_to_write = Vec::new();

        // TODO: might be nicer to have a slice returned and just append in a single location.
        match node {
            NodeType::ByteLiteral { lit } => {
                bytes_to_write.push(lit);
            }
            NodeType::Reference { offset, length } => {
                // copy from the search buffer
                let search_start_index = search_buffer.vec.len() - usize::from(offset);
                let search_stop_index = search_start_index + usize::from(length);
                let b = &search_buffer.vec[search_start_index..search_stop_index];
                bytes_to_write.extend(b);
            }
            NodeType::EndOfStream => {
                // Might not be needed here - might just be a serialisation thing
            }
        };

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
        let expected: Vec<NodeType> = vec![
            NodeType::ByteLiteral { lit: b'a' },
            NodeType::ByteLiteral { lit: b'b' },
            NodeType::Reference {
                offset: 2,
                length: 2,
            },
            NodeType::ByteLiteral { lit: b'c' },
            NodeType::Reference {
                offset: 4,
                length: 3,
            },
            NodeType::Reference {
                offset: 2,
                length: 2,
            },
            NodeType::ByteLiteral { lit: b'a' },
            NodeType::ByteLiteral { lit: b'a' }
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

        assert!(nodes.iter().all(|e| match e {
            NodeType::Reference { length: _, offset } => *offset < 2048,
            _ => true,
        }));
    }

    #[test]
    fn node_list_test_no_trailing_chars() {
        let bytes = vec![b'a', b'b', b'a', b'b', b'b']; // D:

        // (0,0,a), (0,0,b), (2,2,b)
        let expected: Vec<NodeType> = vec![
            NodeType::ByteLiteral { lit: b'a' },
            NodeType::ByteLiteral { lit: b'b' },
            NodeType::Reference {
                offset: 2,
                length: 2,
            },
            NodeType::ByteLiteral { lit: b'b' },
        ];
        let mut nodes = Vec::new();
        build_lz77_node_list(&bytes, |node| nodes.push(node));
        assert_eq!(expected, nodes);
    }
}
