/// The ABI major version, the `MAJOR` of every manager registry table. A client and the manager must agree on it.
pub const AMM_ABI_MAJOR: u32 = 3;
/// The ABI minor version, the `MINOR` and `version` of every manager registry table. It grows when functions are added
/// at the end of a table or fields at the end of a sized struct.
pub const AMM_ABI_MINOR: u32 = 0;
