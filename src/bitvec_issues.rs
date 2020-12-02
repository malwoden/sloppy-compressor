#[cfg(test)]
mod tests {
    use bitvec::prelude::*;

    const DATA_ARRAY: [u8; 100000] = [3; 100000];
    #[test]
    fn bitvec_using_push_higher_cap() {
        let mut vec = bitvec![Msb0, u8;];
        vec.reserve(16000000);

        for d in DATA_ARRAY.iter() {
            let bits = d.view_bits::<Msb0>();
            for b in bits[4..].iter() {
                vec.push(*b);
            }
        }
    }

    #[test]
    fn bitvec_using_push_no_cap() {
        let mut vec = bitvec![Msb0, u8;];

        for d in DATA_ARRAY.iter() {
            let bits = d.view_bits::<Msb0>();
            for b in bits[4..].iter() {
                vec.push(*b);
            }
        }
    }

    #[test]
    fn bitvec_using_append_higher_cap() {
        let mut vec = bitvec![Msb0, u8;];
        vec.reserve(16000000);

        for d in DATA_ARRAY.iter() {
            let bits = d.view_bits::<Msb0>();
            let mut bits_as_vec = bits[4..].to_bitvec();
            vec.append(&mut bits_as_vec);
        }
    }

    #[test]
    fn bitvec_using_append_no_cap() {
        let mut vec = bitvec![Msb0, u8;];

        for d in DATA_ARRAY.iter() {
            let bits = d.view_bits::<Msb0>();
            let mut bits_as_vec = bits[4..].to_bitvec();
            vec.append(&mut bits_as_vec);
        }
    }

    #[test]
    fn bitvec_using_extend_higher_cap() {
        let mut vec = bitvec![Msb0, u8;];
        vec.reserve(16000000);

        for d in DATA_ARRAY.iter() {
            let bits = d.view_bits::<Msb0>();
            vec.extend_from_bitslice(&bits[4..]);
        }
    }

    #[test]
    fn bitvec_using_extend_no_cap() {
        let mut vec = bitvec![Msb0, u8;];

        for d in DATA_ARRAY.iter() {
            let bits = d.view_bits::<Msb0>();
            vec.extend_from_bitslice(&bits[4..]);
        }
    }
}
