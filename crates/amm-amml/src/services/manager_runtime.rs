use std::sync::atomic::AtomicU8;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

use super::{ManagerClock, ManagerListeners, ManagerOverlayHost, ManagerState};
use crate::paths::ManagerPaths;

static RUNTIME: OnceLock<ManagerRuntime> = OnceLock::new();

/// The one runtime of this DLL, made at load: the folders, the clock, the screen code the tick writes and the frames
/// read, the overlay host (the renderer needs one for the process), and while the manager is enabled its shared
/// state and its listeners. The mod callbacks have no context of their own, so they reach it through `get`.
pub struct ManagerRuntime {
    paths: ManagerPaths,
    clock: ManagerClock,
    menu: Arc<AtomicU8>,
    overlay: ManagerOverlayHost,
    state: Mutex<Option<Arc<ManagerState>>>,
    listeners: Mutex<Option<ManagerListeners>>,
}

impl ManagerRuntime {
    /// `menu` is the screen code `overlay` reads for its frames.
    pub fn new(paths: ManagerPaths, clock: ManagerClock, menu: Arc<AtomicU8>, overlay: ManagerOverlayHost) -> Self {
        Self { paths, clock, menu, overlay, state: Mutex::new(None), listeners: Mutex::new(None) }
    }

    /// The screen code of the last tick, shared with the overlay host.
    pub fn menu(&self) -> Arc<AtomicU8> {
        Arc::clone(&self.menu)
    }

    pub fn overlay(&self) -> &ManagerOverlayHost {
        &self.overlay
    }

    /// Keeps `runtime` for the process. The first call wins; later calls return the first.
    pub fn install(runtime: Self) -> &'static Self {
        RUNTIME.get_or_init(|| runtime)
    }

    pub fn get() -> Option<&'static Self> {
        RUNTIME.get()
    }

    pub fn paths(&self) -> &ManagerPaths {
        &self.paths
    }

    pub fn clock(&self) -> ManagerClock {
        self.clock
    }

    /// Keeps the state and the listeners of an enable.
    pub fn activate(&self, state: Arc<ManagerState>, listeners: ManagerListeners) {
        *Self::lock(&self.state) = Some(state);
        *Self::lock(&self.listeners) = Some(listeners);
    }

    /// Takes the state and the listeners of the last enable out, for a disable to drop them outside the locks.
    pub fn deactivate(&self) -> (Option<Arc<ManagerState>>, Option<ManagerListeners>) {
        let state = Self::lock(&self.state).take();
        let listeners = Self::lock(&self.listeners).take();
        (state, listeners)
    }

    /// The shared state while the manager is enabled.
    pub fn state(&self) -> Option<Arc<ManagerState>> {
        Self::lock(&self.state).clone()
    }

    /// How many listeners the manager has now; 0 while it is not enabled.
    pub fn listener_count(&self) -> usize {
        Self::lock(&self.listeners).as_ref().map_or(0, ManagerListeners::len)
    }

    /// Whether the manager holds its panel input lease now.
    pub fn panel_lease_held(&self) -> bool {
        self.state().is_some_and(|state| state.panel_lease_held())
    }

    fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
        mutex.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
