/// A fixed sized container that pops old elements as new ones arrive
#[derive(PartialEq, Debug)]
pub struct ByteBuffer<T> {
    pub vec: Vec<T>,
    limit: usize,
}

impl<T: std::marker::Copy> ByteBuffer<T> {
    // Potential improvements: Use a circular buffer instead, and use a chained iterator when
    // we need to return wrapped segments. Would save on allocations and moves.
    // Performance of swapping direct slice loops with an iter would have to be checked.
    pub fn new(limit: usize) -> ByteBuffer<T> {
        ByteBuffer {
            vec: Vec::with_capacity(limit),
            limit,
        }
    }

    pub fn push_all(&mut self, elements: &[T]) {
        if self.vec.len() + elements.len() > self.limit {
            let count_to_drop = (self.vec.len() + elements.len()) - self.limit;
            self.vec.drain(0..count_to_drop);
        }
        self.vec.extend_from_slice(elements);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_all() {
        let mut search_buffer: ByteBuffer<u8> = ByteBuffer::new(4);
        search_buffer.push_all(&[b'a']);
        search_buffer.push_all(&[b'b', b'c', b'd', b'e']);

        assert_eq!(search_buffer.vec, vec![b'b', b'c', b'd', b'e']);
        search_buffer.push_all(&[b'z']);
        assert_eq!(search_buffer.vec, vec![b'c', b'd', b'e', b'z']);
    }
}
