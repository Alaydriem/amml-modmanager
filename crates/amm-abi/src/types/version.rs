/// The ABI major version. A client and the manager must agree on it.
pub const AMM_ABI_MAJOR: u32 = 2;
/// The ABI minor version. It grows when functions are added at the end of the table or fields at the end of a
/// sized struct.
pub const AMM_ABI_MINOR: u32 = 1;
