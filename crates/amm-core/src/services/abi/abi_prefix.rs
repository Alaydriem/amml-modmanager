/// How many bytes of a boundary struct both sides know. A struct of this kind starts with its own size in bytes as
/// a `u32`; a reader or writer touches only the bytes it may, and the rest keeps the blank value.
pub struct AbiPrefix;

impl AbiPrefix {
    /// The bytes to read or write when the other side gave `declared` as the struct size, `base` is the size of the
    /// fields every version of this major has, and `full` is this side's size of the struct. `None` when `declared`
    /// is smaller than `base`: the struct is too old or not set, and nothing is read or written.
    pub fn len(declared: u32, base: usize, full: usize) -> Option<usize> {
        let declared = usize::try_from(declared).unwrap_or(usize::MAX);
        (declared >= base).then(|| declared.min(full))
    }
}
