use crate::pipeline::ComponentState;

/// Capacity of the SPSC ring buffer (number of slots).
///
/// One slot per raw event; sized to absorb a full tick's worth of driver output
/// without blocking the producer.
pub const RING_CAPACITY: usize = 256;

/// A single slot in the ring buffer.
///
/// `None` represents an empty slot. The runtime drains the buffer each tick
/// and processes events in FIFO order; multiple writes to the same component
/// field within one tick are last-write-wins.
pub type RingSlot = Option<ComponentState>;

/// Header written at the start of the shared memory region.
///
/// Layout (all fields are `usize`-aligned):
/// ```text
/// offset 0: write_index (usize)
/// offset 8: read_index  (usize)
/// offset 16: slots[RING_CAPACITY] (RingSlot array)
/// ```
///
/// Both indices are monotonically increasing. Actual slot index is `index % RING_CAPACITY`.
/// The buffer is full when `write_index - read_index == RING_CAPACITY`.
#[repr(C)]
pub struct ShmHeader {
    pub write_index: usize,
    pub read_index: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_capacity_nonzero() {
        const { assert!(RING_CAPACITY > 0) };
    }

    #[test]
    fn shm_header_size() {
        assert_eq!(std::mem::size_of::<ShmHeader>(), 16);
    }
}
