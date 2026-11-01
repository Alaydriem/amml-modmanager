use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use amm_core::errors::StoreError;
use amm_core::structs::capture::{CaptureImage, CaptureOutcome, PendingCaptures};
use amm_core::structs::write::WriteRequest;
use crossbeam_channel::{Receiver, Sender, bounded};

use crate::services::{CaptureWriter, ConfigFile, IoQueue};
use crate::structs::IoJob;

/// The one thread that writes config files and screenshot PNGs, fed by a bounded `IoQueue`. The outcome of each
/// capture goes back on a channel bounded at `PendingCaptures::CAPACITY`, the most captures in flight, so an
/// outcome only waits there until the manager reads it. Once the queue is closed for shutdown, each queued config
/// write still runs and each queued capture ends with `ok: false` and no encode, so a config write never waits
/// behind PNG encodes in the shutdown flush.
pub struct IoThread {
    queue: IoQueue,
    done_tx: Sender<CaptureOutcome>,
    done_rx: Receiver<CaptureOutcome>,
    worker: Option<JoinHandle<()>>,
}

impl IoThread {
    const TICK: Duration = Duration::from_millis(10);
    const JOIN_POLL: Duration = Duration::from_millis(2);

    pub fn new(queue: IoQueue) -> Result<IoThread, StoreError> {
        let (done_tx, done_rx) = bounded(PendingCaptures::CAPACITY);
        let (worker_queue, worker_done) = (queue.clone(), done_tx.clone());
        let worker = thread::Builder::new()
            .name("amm-io".to_string())
            .spawn(move || Self::run(&worker_queue, &worker_done))
            .map_err(|_| StoreError::Unavailable)?;
        Ok(Self { queue, done_tx, done_rx, worker: Some(worker) })
    }

    pub fn submit(&self, request: WriteRequest) -> Result<(), StoreError> {
        self.queue.try_submit(request)
    }

    /// Queues a capture to encode and write. A full queue is `QueueFull`.
    pub fn submit_capture(&self, image: CaptureImage) -> Result<(), StoreError> {
        self.queue.try_submit_capture(image)
    }

    /// The captures written, or failed, since the last call. It does not block or allocate.
    pub fn written_captures(&self) -> impl Iterator<Item = CaptureOutcome> + '_ {
        self.done_rx.try_iter()
    }

    /// How many more jobs the queue takes now.
    pub fn free_slots(&self) -> usize {
        self.queue.free_slots()
    }

    /// Stops accepting jobs, runs the queued config writes, fails the queued captures, and joins the thread. Gives up and detaches the thread when it
    /// has not finished after `timeout`.
    pub fn shutdown(mut self, timeout: Duration) {
        self.queue.close();
        let Some(worker) = self.worker.take() else {
            return;
        };
        let deadline = Instant::now() + timeout;
        while !worker.is_finished() && Instant::now() < deadline {
            thread::sleep(Self::JOIN_POLL);
        }
        if worker.is_finished() {
            let _ = worker.join();
            // A submit that passed the closed check just before it was set can still land after the thread left.
            while let Ok(job) = self.queue.receiver().try_recv() {
                Self::run_job(job, &self.queue, &self.done_tx);
            }
        } else {
            curia::warn!("I/O thread did not stop in time", { undrained: self.queue.receiver().len() });
        }
    }

    fn run(queue: &IoQueue, done: &Sender<CaptureOutcome>) {
        loop {
            // The queue keeps a sender, so the receiver never disconnects; the closed flag ends the loop.
            if let Ok(job) = queue.receiver().recv_timeout(Self::TICK) {
                Self::run_job(job, queue, done);
            }
            if queue.is_closed() && queue.receiver().is_empty() {
                break;
            }
        }
    }

    fn run_job(job: IoJob, queue: &IoQueue, done: &Sender<CaptureOutcome>) {
        match job {
            IoJob::Config(request) => Self::write(&request),
            IoJob::Capture(image) if queue.is_closed() => Self::drop_capture(&image, done),
            IoJob::Capture(image) => Self::write_capture(&image, done),
        }
    }

    fn drop_capture(image: &CaptureImage, done: &Sender<CaptureOutcome>) {
        curia::info!("screenshot not written, the manager is stopping", {
            sequence: image.sequence,
            path: image.path.display().to_string(),
        });
        Self::send_outcome(CaptureOutcome { sequence: image.sequence, ok: false }, done);
    }

    fn write(request: &WriteRequest) {
        if let Err(error) = ConfigFile::write_atomic(&request.path, &request.text) {
            curia::warn!("config write failed", { path: request.path.display().to_string(), error: error.to_string() });
        }
    }

    fn write_capture(image: &CaptureImage, done: &Sender<CaptureOutcome>) {
        let written = CaptureWriter::write(image);
        if let Err(error) = &written {
            curia::warn!("screenshot not written", {
                sequence: image.sequence,
                path: image.path.display().to_string(),
                format: image.format,
                error: error.to_string(),
            });
        }
        Self::send_outcome(CaptureOutcome { sequence: image.sequence, ok: written.is_ok() }, done);
    }

    fn send_outcome(outcome: CaptureOutcome, done: &Sender<CaptureOutcome>) {
        let sequence = outcome.sequence;
        if done.try_send(outcome).is_err() {
            curia::warn!("screenshot outcome dropped, the outcome channel is full", { sequence: sequence });
        }
    }
}
