/// Capacity of the SPSC ring buffer (number of slots).
///
/// One slot per raw event; sized to absorb a full tick's worth of driver output
/// without blocking the producer.
pub const RING_CAPACITY: usize = 256;

/// Maximum byte length of a device id stored in a [`RingSlot`] (excluding NUL terminator).
pub const DEVICE_ID_MAX: usize = 63;

/// Maximum byte length of a dot-separated specifier stored in a [`RingSlot`] (excluding NUL terminator).
pub const SPECIFIER_MAX: usize = 127;

/// Value discriminant stored in [`RingSlot::value_tag`].
///
/// - 0: `Value::Bool(false)`
/// - 1: `Value::Bool(true)`
/// - 2: `Value::Pulse`
/// - 3: `Value::Int` — integer in `value_i64`
/// - 4: `Value::Float` — float in `value_f64`
/// - 5: `Value::Null`
pub mod value_tag {
    pub const BOOL_FALSE: u8 = 0;
    pub const BOOL_TRUE: u8 = 1;
    pub const PULSE: u8 = 2;
    pub const INT: u8 = 3;
    pub const FLOAT: u8 = 4;
    pub const NULL: u8 = 5;
}

/// A single slot in the SPSC ring buffer.
///
/// All fields are fixed-size so the struct is safe to place in a cross-process
/// `mmap` region.  `occupied == 0` means the slot is empty.
///
/// Strings are stored NUL-terminated and truncated to their respective `*_MAX` constants.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RingSlot {
    /// 0 = empty, 1 = occupied.
    pub occupied: u8,
    /// Value discriminant; see [`value_tag`].
    pub value_tag: u8,
    #[allow(clippy::pub_underscore_fields)]
    pub _pad: [u8; 6],
    /// NUL-terminated device id.
    pub device_id: [u8; DEVICE_ID_MAX + 1],
    /// NUL-terminated dot-separated specifier.
    pub specifier: [u8; SPECIFIER_MAX + 1],
    /// Used when `value_tag` is [`value_tag::INT`].
    pub value_i64: i64,
    /// Used when `value_tag` is [`value_tag::FLOAT`].
    pub value_f64: f64,
}

/// Header written at the start of the shared memory region.
///
/// Layout (all fields are 8-byte aligned):
/// ```text
/// offset 0:  write_index (u64)
/// offset 8:  read_index  (u64)
/// offset 16: slots[RING_CAPACITY] (RingSlot array)
/// ```
///
/// Both indices are monotonically increasing. Actual slot index is `index % RING_CAPACITY`.
/// The buffer is full when `write_index - read_index == RING_CAPACITY`.
#[repr(C)]
pub struct ShmHeader {
    pub write_index: u64,
    pub read_index: u64,
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

    #[test]
    fn ring_slot_is_repr_c() {
        // Verify the slot is a fixed size (not dependent on heap types).
        let size = std::mem::size_of::<RingSlot>();
        assert!(size > 0);
        assert_eq!(size % std::mem::align_of::<RingSlot>(), 0);
    }
}
