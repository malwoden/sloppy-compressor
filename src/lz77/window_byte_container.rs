/// A fixed sized container that pops old elements as new ones arrive
#[derive(PartialEq, Debug)]
pub struct WindowByteContainer<T> {
    pub vec: Vec<T>,
    limit: usize,
}

impl<T: std::marker::Copy> WindowByteContainer<T> {
    pub fn new(limit: usize) -> WindowByteContainer<T> {
        WindowByteContainer {
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
        let mut search_buffer: WindowByteContainer<u8> = WindowByteContainer::new(4);
        search_buffer.push_all(&[b'a']);
        search_buffer.push_all(&[b'b', b'c', b'd', b'e']);

        assert_eq!(search_buffer.vec, vec![b'b', b'c', b'd', b'e']);
        search_buffer.push_all(&[b'z']);
        assert_eq!(search_buffer.vec, vec![b'c', b'd', b'e', b'z']);
    }
}
