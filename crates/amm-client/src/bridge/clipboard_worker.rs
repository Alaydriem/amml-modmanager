use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;

use amm_core::structs::frame::LayerKind;
use crossbeam_channel::{Receiver, Sender, TrySendError};

use super::{ClipboardBackend, ClipboardRequest, ClipboardThread};

/// The clipboard of one mod. Requests go to a worker thread through a queue of `QUEUE`, so the render thread
/// never waits on the clipboard; a request that finds the queue full is dropped. Each layer gets its paste
/// result back through its own channel of one, and a result that finds that channel full is dropped. The thread
/// starts on the first request and is joined on drop.
pub struct ClipboardWorker {
    backend: ClipboardBackend,
    requests: Option<Sender<ClipboardRequest>>,
    inbox: Receiver<ClipboardRequest>,
    paste_senders: [Sender<String>; LayerKind::COUNT],
    pastes: [Receiver<String>; LayerKind::COUNT],
    shutdown: Arc<AtomicBool>,
    thread: OnceLock<Option<JoinHandle<()>>>,
    full_logged: AtomicBool,
    oversize_logged: AtomicBool,
}

impl ClipboardWorker {
    /// Requests that can wait for the worker thread.
    pub const QUEUE: usize = 4;
    /// The longest text a copy puts on the clipboard, in bytes. A longer copy is refused.
    pub const MAX_COPY_BYTES: usize = 1 << 20;
    /// The longest text a paste gives a layer, in bytes. Longer clipboard text is cut on a char boundary.
    pub const MAX_PASTE_BYTES: usize = 64 << 10;

    pub fn new(backend: ClipboardBackend) -> Self {
        let (requests, inbox) = crossbeam_channel::bounded(Self::QUEUE);
        let (hud_sender, hud) = crossbeam_channel::bounded(1);
        let (overlay_sender, overlay) = crossbeam_channel::bounded(1);
        Self {
            backend,
            requests: Some(requests),
            inbox,
            paste_senders: [hud_sender, overlay_sender],
            pastes: [hud, overlay],
            shutdown: Arc::new(AtomicBool::new(false)),
            thread: OnceLock::new(),
            full_logged: AtomicBool::new(false),
            oversize_logged: AtomicBool::new(false),
        }
    }

    /// Queues `text` for the clipboard. False when the text is over `MAX_COPY_BYTES` or the queue is full.
    pub fn copy(&self, text: String) -> bool {
        if text.len() > Self::MAX_COPY_BYTES {
            if !self.oversize_logged.swap(true, Ordering::Relaxed) {
                curia::warn!("copy over the clipboard limit, not copied", {
                    bytes: text.len(),
                    limit: Self::MAX_COPY_BYTES,
                });
            }
            return false;
        }
        self.send(ClipboardRequest::Copy(text))
    }

    /// Queues a read of the clipboard for `layer`, whose text `take_paste` gives back later. False when the
    /// queue is full.
    pub fn request_paste(&self, layer: LayerKind) -> bool {
        self.send(ClipboardRequest::Paste(layer))
    }

    /// The text of the last paste `layer` asked for, once it was read.
    pub fn take_paste(&self, layer: LayerKind) -> Option<String> {
        self.pastes[layer.slot()].try_recv().ok()
    }

    /// Requests in the queue that the worker thread has not taken yet.
    pub fn pending(&self) -> usize {
        self.inbox.len()
    }

    /// Tells the worker thread to stop after the request it is in, and takes no more requests. The drop joins
    /// the thread.
    pub(crate) fn stop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        self.requests = None;
    }

    fn send(&self, request: ClipboardRequest) -> bool {
        let Some(requests) = self.requests.as_ref() else { return false };
        if !self.started() {
            return false;
        }
        match requests.try_send(request) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => {
                if !self.full_logged.swap(true, Ordering::Relaxed) {
                    curia::warn!("clipboard queue full, request dropped", { queue: Self::QUEUE });
                }
                false
            }
            Err(TrySendError::Disconnected(_)) => false,
        }
    }

    fn started(&self) -> bool {
        self.thread.get_or_init(|| self.spawn()).is_some()
    }

    fn spawn(&self) -> Option<JoinHandle<()>> {
        let thread = ClipboardThread::new(
            self.backend.clone(),
            self.inbox.clone(),
            self.paste_senders.clone(),
            Arc::clone(&self.shutdown),
        );
        match std::thread::Builder::new().name("amm-clipboard".to_owned()).spawn(move || thread.run()) {
            Ok(handle) => Some(handle),
            Err(error) => {
                curia::error!("clipboard thread not started, clipboard off", { error: error.to_string() });
                None
            }
        }
    }
}

impl Drop for ClipboardWorker {
    fn drop(&mut self) {
        self.stop();
        if let Some(handle) = self.thread.take().flatten()
            && handle.join().is_err()
        {
            curia::error!("clipboard thread panicked");
        }
    }
}
