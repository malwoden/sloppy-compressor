use std::collections::VecDeque;

/// A fixed sized container that pops old elements as new ones arrive
#[derive(PartialEq, Debug)]
pub struct WindowByteContainer<T> {
    pub vec: VecDeque<T>,
    limit: usize,
}

impl<T: std::marker::Copy> WindowByteContainer<T> {
    pub fn new(limit: usize) -> WindowByteContainer<T> {
        WindowByteContainer {
            vec: VecDeque::with_capacity(limit),
            limit,
        }
    }

    pub fn push(&mut self, element: T) {
        if self.vec.len() == self.limit {
            self.vec.pop_front();
        }
        self.vec.push_back(element);
    }

    pub fn push_all(&mut self, elements: &[T]) {
        while self.vec.len() + elements.len() > self.limit {
            self.vec.pop_front();
        }
        for e in elements {
            self.vec.push_back(*e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_all() {
        let mut search_buffer: WindowByteContainer<u8> = WindowByteContainer::new(4);
        search_buffer.push(b'a');
        search_buffer.push_all(&[b'b', b'c', b'd', b'e']);

        assert_eq!(search_buffer.vec, vec![b'b', b'c', b'd', b'e']);
        search_buffer.push(b'z');
        assert_eq!(search_buffer.vec, vec![b'c', b'd', b'e', b'z']);
    }
}
