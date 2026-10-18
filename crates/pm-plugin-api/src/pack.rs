//! A pointer and a length in one number, as a function crossing the boundary returns them.

/// `pointer` and `length` packed into one `i64`, the pointer in the high half.
pub fn pack(pointer: u32, length: u32) -> i64 {
    (i64::from(pointer) << 32) | i64::from(length)
}

/// The pointer and the length that [`pack`] put in `packed`.
pub fn unpack(packed: i64) -> (u32, u32) {
    ((packed >> 32) as u32, packed as u32)
}
