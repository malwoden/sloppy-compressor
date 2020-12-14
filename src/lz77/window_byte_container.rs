use std::{
    cmp,
    collections::{HashMap, VecDeque},
    hash::{BuildHasherDefault, Hasher},
};

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

#[derive(PartialEq, Debug)]
pub struct ByteWindowAdvance<'a> {
    pub evicted: &'a [u8],
    pub admitted: &'a [u8],
    pub window: &'a [u8],
}

// Maybe add an implementation for converting to a &[u8]?

/// Wrapper for a window view over a byte slice
#[derive(PartialEq, Debug)]
pub struct ByteWindow<'a> {
    bytes: &'a [u8],
    max_window_size: usize,
    current_index: usize,
}

impl<'a> ByteWindow<'a> {
    pub fn with_max_window_size(bytes: &'a [u8], max_window_size: usize) -> Self {
        ByteWindow {
            bytes,
            max_window_size,
            current_index: 0,
        }
    }

    /// Moves the end-of-window pointer by count.
    /// count is allowed to exceed the size of the underlying byte slice - this function and
    /// the window() function will return empty slices if the count + max_slice_length exceeds
    /// the byte slice length.
    ///
    /// Returned struct contains the bytes that were evicted and admitted from/to the window.
    /// ```
    /// use sloppycomp::lz77::window_byte_container::{ByteWindow, ByteWindowAdvance};
    /// let bytes = [b'b', b'c', b'd', b'e'];
    /// let mut byte_window = ByteWindow::with_max_window_size(&bytes, 2);
    /// let advancement = byte_window.advance(1);
    /// assert_eq!(
    ///     ByteWindowAdvance {
    ///         evicted: &[],
    ///         admitted: &[b'b'],
    ///         window: &[b'b']
    ///     },
    ///     advancement
    /// );
    /// ```
    ///
    /// It is possible for the same byte to be emitted and evicted in a single operation
    /// if the count exceeds the max_window_size of the ByteWindow.
    /// ```
    /// use sloppycomp::lz77::window_byte_container::{ByteWindow, ByteWindowAdvance};
    /// let bytes = [b'b', b'c', b'd', b'e'];
    /// let mut byte_window = ByteWindow::with_max_window_size(&bytes, 2);
    ///
    /// let door = byte_window.advance(5);
    /// assert_eq!(
    ///     ByteWindowAdvance {
    ///         evicted: &[b'b', b'c', b'd'],
    ///         admitted: &[b'b', b'c', b'd', b'e'],
    ///         window: &[b'e']
    ///     },
    ///     door
    /// );
    /// ```
    pub fn advance(&mut self, count: usize) -> ByteWindowAdvance<'a> {
        let new_pointer = self.current_index + count;
        self.advance_to_pointer(new_pointer)
    }

    pub fn advance_to_pointer(&mut self, pointer: usize) -> ByteWindowAdvance<'a> {
        let new_start_index = pointer.saturating_sub(self.max_window_size);
        let old_start_index = self.current_index.saturating_sub(self.max_window_size);
        let end_index = cmp::min(self.bytes.len(), pointer);

        let window = if new_start_index < end_index {
            &self.bytes[new_start_index..end_index]
        } else {
            &[]
        };

        let admitted = if self.current_index < self.bytes.len() {
            &self.bytes[self.current_index..end_index]
        } else {
            &[]
        };

        let evicted = if old_start_index < new_start_index && old_start_index < self.bytes.len() {
            &self.bytes[old_start_index..new_start_index]
        } else {
            &[]
        };

        self.current_index = pointer;

        ByteWindowAdvance {
            evicted,
            admitted,
            window,
        }
    }

    pub fn window(&self) -> &'a [u8] {
        let start_index = self.current_index.saturating_sub(self.max_window_size);
        let end_index = cmp::min(self.bytes.len(), self.current_index);
        if start_index < self.bytes.len() {
            &self.bytes[start_index..end_index]
        } else {
            &[]
        }
    }
}

/// ByteWindow that also tracks the locations of each byte in an O(1) lookup
///
/// After every admission and eviction the internal hashmap of byte values to indexes
/// is updated. Note that the indexes stored in this location are not restricted to the
/// visible byte window, they are global across the whole original byte slice.
/// This means we don't have to update values in the collection as we slide the window.
#[derive(PartialEq, Debug)]
pub struct IndexableByteWindow<'a> {
    window: ByteWindow<'a>,
    byte_locations: HashMap<u8, VecDeque<usize>, U8HasherBuilder>,
}

impl<'a> IndexableByteWindow<'a> {
    pub fn with_max_window_size(bytes: &'a [u8], max_window_size: usize) -> Self {
        IndexableByteWindow {
            window: ByteWindow::with_max_window_size(bytes, max_window_size),
            byte_locations:
                HashMap::<u8, VecDeque<usize>, U8HasherBuilder>::with_capacity_and_hasher(
                    256,
                    U8HasherBuilder::default(),
                ),
        }
    }

    pub fn advance(&mut self, count: usize) -> ByteWindowAdvance<'a> {
        let result = self.window.advance(count);
        self.update_byte_location_table(
            result.admitted,
            self.window.current_index - count,
            result.evicted,
        );
        result
    }

    pub fn advance_to_pointer(&mut self, pointer: usize) -> ByteWindowAdvance<'a> {
        self.advance(pointer - self.window.current_index)
    }

    pub fn window(&self) -> &'a [u8] {
        self.window.window()
    }

    fn update_byte_location_table(
        &mut self,
        admitted: &[u8],
        admission_offset: usize,
        evicted: &[u8],
    ) {
        admitted.iter().enumerate().for_each(|(i, u)| {
            self.byte_locations
                .entry(*u)
                .or_default()
                .push_back(i + admission_offset);
        });

        evicted.iter().for_each(|u| {
            // we are moving through the slice from lowest index first, so we know
            // that if we are removing an element from a given key, the element to
            // remove must be first in the deque.
            self.byte_locations.entry(*u).and_modify(|v| {
                v.pop_front();
            });
        });
    }

    /// Returns a collection of byte values to their known location within the byte slice.
    pub fn byte_locations(&self) -> &HashMap<u8, VecDeque<usize>, U8HasherBuilder> {
        &self.byte_locations
    }

    /// Translates a location within the byte slice to a location within the
    /// current visible window.
    pub fn location_to_window_index(&self, location: usize) -> usize {
        let offset = self
            .window
            .current_index
            .saturating_sub(self.window.max_window_size);

        if location > self.window.current_index || location < offset {
            panic!("Location must be indexable within the current visible window");
        }
        location - offset
    }
}

// Custom hasher optimised for the u8 keys of the IndexableByteWindow's map.
#[derive(Default)]
pub struct U8Hasher {
    hash: u8,
}

impl Hasher for U8Hasher {
    fn finish(&self) -> u64 {
        u64::from(self.hash)
    }

    fn write(&mut self, bytes: &[u8]) {
        self.hash = bytes[0]
    }
}

type U8HasherBuilder = BuildHasherDefault<U8Hasher>;

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

#[cfg(test)]
mod byte_window_tests {
    use super::*;

    #[test]
    fn advance_less_than_window_size() {
        let bytes = [b'b', b'c', b'd', b'e'];
        let mut byte_window = ByteWindow::with_max_window_size(&bytes, 2);

        assert_eq!([] as [u8; 0], byte_window.window());
        assert_window_advance(&mut byte_window, &[], &[b'b'], &[b'b']);
        assert_window_advance(&mut byte_window, &[], &[b'c'], &[b'b', b'c']);
        assert_window_advance(&mut byte_window, &[b'b'], &[b'd'], &[b'c', b'd']);
        assert_window_advance(&mut byte_window, &[b'c'], &[b'e'], &[b'd', b'e']);
        assert_window_advance(&mut byte_window, &[b'd'], &[], &[b'e']);
        assert_window_advance(&mut byte_window, &[b'e'], &[], &[]);
        assert_window_advance(&mut byte_window, &[], &[], &[]);
    }

    #[test]
    fn advance_past_end_of_window() {
        let bytes = [b'b', b'c', b'd', b'e'];
        let mut byte_window = ByteWindow::with_max_window_size(&bytes, 2);

        let door = byte_window.advance(5);
        assert_eq!(
            ByteWindowAdvance {
                evicted: &[b'b', b'c', b'd'],
                admitted: &[b'b', b'c', b'd', b'e'],
                window: &[b'e']
            },
            door
        );
        assert_eq!([b'e'], byte_window.window());
    }

    #[test]
    fn advance_past_end_of_window_byte_locations() {
        let bytes = [b'b', b'c', b'd', b'e'];
        let mut byte_window = IndexableByteWindow::with_max_window_size(&bytes, 2);

        byte_window.advance(5);

        assert_eq!(
            byte_window.byte_locations().get(&b'e').unwrap().as_slices(),
            (&[3][..], &[][..])
        );
    }

    #[test]
    fn location_to_window_index() {
        let bytes = [b'b', b'c', b'd', b'e'];
        let mut byte_window = IndexableByteWindow::with_max_window_size(&bytes, 2);

        assert_eq!(0, byte_window.location_to_window_index(0));

        byte_window.advance(4);

        assert_eq!(0, byte_window.location_to_window_index(2));
        assert_eq!(1, byte_window.location_to_window_index(3));
    }

    #[test]
    fn advance_to_pointer() {
        let bytes = [b'b', b'c', b'd', b'e'];
        let mut byte_window = ByteWindow::with_max_window_size(&bytes, 2);

        let door = byte_window.advance_to_pointer(3);
        assert_eq!(
            ByteWindowAdvance {
                evicted: &[b'b'],
                admitted: &[b'b', b'c', b'd'],
                window: &[b'c', b'd']
            },
            door
        );
        assert_eq!([b'c', b'd'], byte_window.window());
    }

    fn assert_window_advance(
        byte_window: &mut ByteWindow,
        evicted: &[u8],
        admitted: &[u8],
        window: &[u8],
    ) {
        let door = byte_window.advance(1);
        assert_eq!(
            ByteWindowAdvance {
                evicted,
                admitted,
                window
            },
            door
        );
        assert_eq!(window, byte_window.window());
    }
}
