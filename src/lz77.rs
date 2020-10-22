use std::io::prelude::*;
use std::{cmp, fs::File};

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
        assert_eq!(expected, build_lz77_node_list(&bytes));
    }

    #[test]
    fn build_lz77_node_list_test_no_trailing_chars() {
        let bytes = vec![b'a', b'b', b'a', b'b']; // D:

        // (0,0,a), (0,0,b), (2,1,b)
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
                length: 1,
                char: b'b',
            },
        ];
        assert_eq!(expected, build_lz77_node_list(&bytes));
    }
}
#[derive(PartialEq, Debug)]
struct Node {
    offset: usize,
    length: usize,
    char: u8,
}

pub fn compress(file: &mut File) {
    let mut file_bytes = Vec::new();
    file.read_to_end(&mut file_bytes)
        .expect("Error on file read");
}

fn build_lz77_node_list(to_compress: &[u8]) -> Vec<Node> {
    let search_window_size = 32;
    let prefix_window_size = 32;

    let mut byte_ptr = 0;
    let mut lz77_nodes = vec![];

    println!("{:?}", to_compress);
    loop {
        let c = to_compress[byte_ptr];
        let search_slice_start_index = byte_ptr.saturating_sub(search_window_size);
        let search_slice_end_index = byte_ptr;

        let prefix_slice_start_index = byte_ptr + 1;
        let prefix_slice_end_index = cmp::min(
            byte_ptr.saturating_add(prefix_window_size),
            to_compress.len(),
        );

        let search_slice = &to_compress[search_slice_start_index..search_slice_end_index];
        let prefix_slice = &to_compress[prefix_slice_start_index..prefix_slice_end_index];
        println!(
            "search slice: {:?}, byte: {:?}, prefix_slice: {:?}",
            search_slice, c, prefix_slice,
        );

        let node = calculate_node(c, search_slice, prefix_slice);
        println!("Node: {:?}", node);
        byte_ptr += 1 + node.length; // advance the byte pointer 1 past the position of char literal in the node
        lz77_nodes.push(node);

        if byte_ptr > to_compress.len() - 1 {
            break;
        }
    }

    lz77_nodes
}

fn calculate_node(c: u8, left_slice: &[u8], right_slice: &[u8]) -> Node {
    let mut offset = 0;
    let mut length = 0;

    for i in (0..left_slice.len()).rev() {
        if c == left_slice[i] {
            let series_match = find_length_of_series_match(&left_slice[i + 1..], right_slice);
            println!("series match: {:?}", series_match);
            if series_match > length {
                offset = left_slice.len() - i;
                length = series_match + 1; // + 1 to include the 'c' char
            }
        }
    }

    let next_char;
    if length == 0 {
        // offset and length are 0 - this is a char literal node
        next_char = c;
    } else {
        if length > right_slice.len() {
            // we always need to have a char in the Node, but if we find a match up to the end of the
            // right slice, we have no 'next' char to set. So we shorten the matched pattern by 1 char
            // and set that final char as the next char for this node
            length -= 1;
        }

        next_char = right_slice[length - 1]
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

// pub fn decompress(file: File) -> () {}
