use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use amm_core::structs::frame::LayerKind;
use crossbeam_channel::{Receiver, Sender, TrySendError};

use super::{ClipboardBackend, ClipboardFault, ClipboardRequest, ClipboardWorker};

/// The body of the clipboard worker thread: it takes requests in order until the request channel disconnects or
/// shutdown is set. A busy clipboard is tried `ATTEMPTS` times, `RETRY` apart; with the 15.6 ms Windows timer a
/// sleep can last a full tick, so one request holds the thread for about 60 ms. Once shutdown is set no retry
/// starts, so a join waits at most for one sleep and one clipboard access.
pub struct ClipboardThread {
    backend: ClipboardBackend,
    requests: Receiver<ClipboardRequest>,
    pastes: [Sender<String>; LayerKind::COUNT],
    shutdown: Arc<AtomicBool>,
    busy_logged: bool,
    failed_logged: bool,
    untaken_logged: bool,
}

impl ClipboardThread {
    const ATTEMPTS: u32 = 5;
    const RETRY: Duration = Duration::from_millis(10);

    pub fn new(
        backend: ClipboardBackend,
        requests: Receiver<ClipboardRequest>,
        pastes: [Sender<String>; LayerKind::COUNT],
        shutdown: Arc<AtomicBool>,
    ) -> Self {
        Self { backend, requests, pastes, shutdown, busy_logged: false, failed_logged: false, untaken_logged: false }
    }

    pub fn run(mut self) {
        while let Ok(request) = self.requests.recv() {
            if self.shutdown.load(Ordering::SeqCst) {
                return;
            }
            match request {
                ClipboardRequest::Copy(text) => {
                    let result = self.attempt(|backend| backend.write(&text));
                    self.report(result);
                }
                ClipboardRequest::Paste(layer) => {
                    let result = self.attempt(ClipboardBackend::read);
                    if let Some(text) = self.report(result) {
                        self.deliver(layer, text);
                    }
                }
            }
        }
    }

    // Stops early when shutdown is set, so a drop does not wait out the retries
    fn attempt<T>(
        &self,
        mut access: impl FnMut(&ClipboardBackend) -> Result<T, ClipboardFault>,
    ) -> Result<T, ClipboardFault> {
        let mut result = access(&self.backend);
        for _ in 1..Self::ATTEMPTS {
            if !matches!(result, Err(ClipboardFault::Busy)) || self.shutdown.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(Self::RETRY);
            result = access(&self.backend);
        }
        result
    }

    fn report<T>(&mut self, result: Result<T, ClipboardFault>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(ClipboardFault::Busy) => {
                if !std::mem::replace(&mut self.busy_logged, true) {
                    curia::warn!("clipboard busy, request dropped", { attempts: Self::ATTEMPTS });
                }
                None
            }
            Err(ClipboardFault::Failed) => {
                if !std::mem::replace(&mut self.failed_logged, true) {
                    curia::warn!("clipboard text not read or written, request dropped");
                }
                None
            }
        }
    }

    fn deliver(&mut self, layer: LayerKind, mut text: String) {
        if text.is_empty() {
            return;
        }
        let end = text.floor_char_boundary(ClipboardWorker::MAX_PASTE_BYTES);
        text.truncate(end);
        if let Err(TrySendError::Full(_)) = self.pastes[layer.slot()].try_send(text)
            && !std::mem::replace(&mut self.untaken_logged, true)
        {
            curia::warn!("paste dropped, the layer has not taken the last one", { layer: layer.slot() });
        }
    }
}
