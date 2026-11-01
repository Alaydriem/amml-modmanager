/// The allocations, reallocations and bytes allocated by one thread.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ThreadCount {
    pub allocations: usize,
    pub reallocations: usize,
    pub bytes_allocated: usize,
}
