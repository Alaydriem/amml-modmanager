/// The addresses of the functions the overlay hooks, read from the vtables of probe objects. Every object of the
/// same implementation shares them, so the hooks also see the game's swap chain and queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeTargets {
    pub present: usize,
    pub resize_buffers: usize,
    pub present1: usize,
    pub resize_buffers1: usize,
    /// `ID3D12CommandQueue::ExecuteCommandLists`, or `None` when no probe D3D12 queue could be made. Without it the
    /// D3D11 path still draws and the D3D12 path cannot.
    pub execute_command_lists: Option<usize>,
}
