use std::mem::size_of;

use bitvec::prelude::*;

use std::convert::TryFrom;

use super::nodes::NodeType;

const U16_BIT_SIZE: usize = size_of::<u16>() * 8;

pub fn serailise_nodes(nodes: &Vec<NodeType>) -> BitVec<Msb0, u8> {
    let mut vec = bitvec![Msb0, u8;];
    // Don't reserve here as a bug in bit-vec results in slower extend/append ops.

    for node in nodes {
        match node {
            NodeType::ByteLiteral { lit } => {
                // literal byte - push '0' followed by 8 bits for the byte val
                vec.push(false);
                let literal = BitSlice::<Msb0, u8>::from_element(lit);
                append_bitvecs(&mut vec, &literal.to_bitvec());
            }
            NodeType::Reference { offset, length } => {
                // offset / length reference
                vec.push(true);
                let x = offset.view_bits::<Msb0>();
                if *offset < 128 {
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
                let length_encoded = serialise_length(*length);
                append_bitvecs(&mut vec, &length_encoded);
            }
            NodeType::EndOfStream => {}
        }
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

pub fn deserialise_nodes(file_bytes: &Vec<u8>) -> Vec<NodeType> {
    let end_of_stream_marker = bits![Msb0, u8; 1, 1, 0, 0, 0, 0, 0, 0, 0];

    let mut nodes: Vec<NodeType> = vec![];
    let bit_view = file_bytes.view_bits::<Msb0>();

    let mut bitstream_offset = 0;
    while bitstream_offset < bit_view.len() {
        let is_reference_node = bit_view[bitstream_offset];
        bitstream_offset += 1;

        if !is_reference_node {
            // next 8 bits will be a literal byte node
            let byte_literal = &bit_view[bitstream_offset..bitstream_offset + 8];
            nodes.push(NodeType::ByteLiteral {
                lit: slice_to_byte(&byte_literal),
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

            nodes.push(NodeType::Reference { length, offset });
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
pub fn append_end_marker<O, T>(encoding: &mut BitVec<O, T>)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serailise_nodes_handles_literals_and_refs() {
        let nodes: Vec<NodeType> = vec![
            NodeType::ByteLiteral { lit: b'a' },
            NodeType::ByteLiteral { lit: b'b' },
            NodeType::Reference {
                offset: 2,
                length: 2,
            },
            NodeType::ByteLiteral { lit: b'b' },
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
            0, 0, 1, 1, 0, 0, 0, 1, 0,
        ];
        assert_eq!(expected, serailise_nodes(&nodes));
    }

    #[test]
    fn serailise_nodes_large_lengths() {
        let nodes = vec![
            NodeType::Reference {
                offset: 17,
                length: 8,
            },
            NodeType::ByteLiteral { lit: b'a' },
        ];
        assert_eq!(
            bitvec![1, 1, 0, 0, 1, 0, 0, 0, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1],
            serailise_nodes(&nodes)
        );

        let nodes = vec![
            NodeType::Reference {
                offset: 17,
                length: 24,
            },
            NodeType::ByteLiteral { lit: b'a' },
        ];
        assert_eq!(
            bitvec![
                1, 1, 0, 0, 1, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 0,
                0, 1
            ],
            serailise_nodes(&nodes)
        );
    }

    #[test]
    fn serailise_nodes_size_experiment() {
        // Q: What is most efficient: 3 raw bytes or 1 raw byte and a 2 byte-len node ref?

        let three_raw_bytes = vec![
            NodeType::ByteLiteral { lit: b'a' },
            NodeType::ByteLiteral { lit: b'a' },
            NodeType::ByteLiteral { lit: b'a' },
        ];
        assert_eq!(
            bitvec![
                0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 0, 0, 1,
            ],
            serailise_nodes(&three_raw_bytes)
        );

        let two_length_node_ref = vec![
            NodeType::Reference {
                offset: 2,
                length: 2,
            },
            NodeType::ByteLiteral { lit: b'a' },
        ];
        assert_eq!(
            bitvec![1, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1,],
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
        let nodes: Vec<NodeType> = vec![
            NodeType::ByteLiteral { lit: b'a' },
            NodeType::ByteLiteral { lit: b'b' },
            NodeType::Reference {
                offset: 2,
                length: 2,
            },
        ];
        let mut serialised = serailise_nodes(&nodes);
        append_end_marker(&mut serialised);
        let deserialised = deserialise_nodes(&serialised.into());
        assert_eq!(nodes, deserialised);
    }
}
