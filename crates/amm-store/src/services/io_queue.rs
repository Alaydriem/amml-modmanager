use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use amm_core::errors::StoreError;
use amm_core::structs::capture::CaptureImage;
use amm_core::structs::write::WriteRequest;
use crossbeam_channel::{Receiver, Sender, TrySendError, bounded};

use crate::structs::IoJob;

/// A bounded queue of I/O jobs: config writes and screenshot captures share it. The bound is the capacity given
/// to `new`; a job past it is refused and counted, never queued. Clones share one queue.
#[derive(Clone)]
pub struct IoQueue {
    tx: Sender<IoJob>,
    rx: Receiver<IoJob>,
    refused: Arc<AtomicU64>,
    closed: Arc<AtomicBool>,
}

impl IoQueue {
    pub fn new(capacity: usize) -> IoQueue {
        let (tx, rx) = bounded(capacity);
        Self { tx, rx, refused: Arc::new(AtomicU64::new(0)), closed: Arc::new(AtomicBool::new(false)) }
    }

    pub fn try_submit(&self, request: WriteRequest) -> Result<(), StoreError> {
        self.try_submit_job(IoJob::Config(request))
    }

    /// Queues a capture to encode and write. A full queue is `QueueFull`: the capture fails, it is not retried.
    pub fn try_submit_capture(&self, image: CaptureImage) -> Result<(), StoreError> {
        self.try_submit_job(IoJob::Capture(image))
    }

    /// How many more jobs the queue takes now. Only the writer thread takes jobs out, so for the one producer
    /// this only grows until its next submit.
    pub fn free_slots(&self) -> usize {
        self.tx.capacity().unwrap_or(usize::MAX).saturating_sub(self.tx.len())
    }

    pub fn receiver(&self) -> &Receiver<IoJob> {
        &self.rx
    }

    /// How many jobs were refused because the queue was full.
    pub fn refused(&self) -> u64 {
        self.refused.load(Ordering::Relaxed)
    }

    /// Refuses every later submit with `Unavailable`.
    pub fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    fn try_submit_job(&self, job: IoJob) -> Result<(), StoreError> {
        if self.is_closed() {
            return Err(StoreError::Unavailable);
        }
        match self.tx.try_send(job) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => {
                self.refused.fetch_add(1, Ordering::Relaxed);
                Err(StoreError::QueueFull)
            }
            Err(TrySendError::Disconnected(_)) => Err(StoreError::Unavailable),
        }
    }
}
