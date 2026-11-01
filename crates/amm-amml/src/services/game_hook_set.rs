use std::sync::{Mutex, MutexGuard, PoisonError};

use amml_sdk::{HookHandle, SdkError};

use crate::traits::HookHost;
use crate::types::{HookGroup, HookSpec};

/// The game hooks the manager installed, by group, so disable removes them in its order.
pub struct GameHookSet {
    // at most CAPACITY, one per spec
    installed: Mutex<Vec<(HookGroup, HookHandle)>>,
}

impl GameHookSet {
    pub const CAPACITY: usize = 16;

    pub fn new() -> Self {
        Self { installed: Mutex::new(Vec::with_capacity(Self::CAPACITY)) }
    }

    /// Every required entry must resolve before any hook goes in; a missing optional entry turns off only its
    /// own feature, with one log line. A hook the loader refuses removes every hook of this set again.
    pub fn install<H: HookHost>(&self, host: &H, specs: &[HookSpec]) -> Result<(), SdkError> {
        if self.lock().len() + specs.len() > Self::CAPACITY {
            return Err(SdkError::Mod(format!("more than {} game hooks", Self::CAPACITY)));
        }
        let mut targets = Vec::with_capacity(specs.len());
        for spec in specs {
            match host.resolve(spec.entry) {
                Ok(target) => targets.push((target, spec)),
                Err(error) if spec.required => return Err(error),
                Err(error) => host.warn(&format!("{} did not resolve ({error}); {} is off", spec.entry, spec.feature)),
            }
        }
        for (target, spec) in targets {
            match host.hook(target, spec) {
                Ok(handle) => self.lock().push((spec.group, handle)),
                Err(error) => {
                    self.remove_all(host);
                    return Err(error);
                }
            }
        }
        Ok(())
    }

    /// Removes the hooks of `group`.
    pub fn remove<H: HookHost>(&self, host: &H, group: HookGroup) {
        let handles: Vec<HookHandle> = {
            let mut installed = self.lock();
            let handles = installed.iter().filter(|(of, _)| *of == group).map(|(_, handle)| *handle).collect();
            installed.retain(|(of, _)| *of != group);
            handles
        };
        Self::unhook(host, handles);
    }

    pub fn remove_all<H: HookHost>(&self, host: &H) {
        let handles: Vec<HookHandle> = self.lock().drain(..).map(|(_, handle)| handle).collect();
        Self::unhook(host, handles);
    }

    fn unhook<H: HookHost>(host: &H, handles: Vec<HookHandle>) {
        for handle in handles {
            if let Err(error) = host.unhook(handle) {
                host.warn(&format!("unhook failed: {error}"));
            }
        }
    }

    fn lock(&self) -> MutexGuard<'_, Vec<(HookGroup, HookHandle)>> {
        self.installed.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Default for GameHookSet {
    fn default() -> Self {
        Self::new()
    }
}
