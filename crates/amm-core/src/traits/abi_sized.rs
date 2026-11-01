/// A struct that crosses the C boundary and may get new fields at its end in a later minor version. Its first
/// field is its size in bytes as a `u32`, at offset 0, and it ends with a field, never with padding, so a later
/// field always grows its size. `AbiPrefix` decides how many bytes of it two sides share.
///
/// Every field takes any byte pattern as a valid value: integers, floats, raw pointers, an `Option` of a function
/// pointer, and arrays and structs of these. A reader keeps the bytes the other side sent as they are, so a field
/// type with invalid bit patterns, such as `bool`, a Rust enum or a reference, must not be part of such a struct.
pub trait AbiSized: Copy {
    /// The bytes from the start of the struct to the end of the last field of minor version 0 of this major.
    const BASE_SIZE: usize;

    /// A value with every field 0 or null and the size field set to `size_of::<Self>()`.
    fn blank() -> Self;
}
