use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// Holds the lock of a memory clipboard on its own thread until it is dropped, so it can live inside a handler.
pub struct ClipboardHold {
    release: Option<SyncSender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl ClipboardHold {
    /// Returns once the lock is held.
    pub fn new(memory: &Arc<Mutex<String>>) -> Self {
        let memory = Arc::clone(memory);
        let (locked_sender, locked) = mpsc::sync_channel(1);
        let (release, released) = mpsc::sync_channel::<()>(1);
        let thread = std::thread::spawn(move || {
            let held = memory.lock().unwrap_or_else(|error| panic!("memory clipboard: {error}"));
            locked_sender.send(()).unwrap_or_else(|error| panic!("hold signal: {error}"));
            // the sender is dropped with the hold, which ends this wait
            let _ = released.recv();
            drop(held);
        });
        locked.recv().unwrap_or_else(|error| panic!("hold thread: {error}"));
        Self { release: Some(release), thread: Some(thread) }
    }
}

impl Drop for ClipboardHold {
    fn drop(&mut self) {
        self.release = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
