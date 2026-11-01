use std::sync::{Arc, Mutex};

use amm_client::bridge::{ClipboardBackend, ClipboardWorker};
use amm_core::structs::frame::LayerKind;

use crate::support::ClipboardPass;

#[test]
fn copied_text_reaches_the_clipboard() {
    let memory = Arc::new(Mutex::new(String::new()));
    let worker = ClipboardWorker::new(ClipboardBackend::Memory(Arc::clone(&memory)));

    assert!(worker.copy("hello".to_owned()));

    assert!(ClipboardPass::wait_for_text(&memory, "hello"));
}

#[test]
fn a_paste_returns_the_clipboard_text_to_the_layer_that_asked() {
    let memory = Arc::new(Mutex::new("hello".to_owned()));
    let worker = ClipboardWorker::new(ClipboardBackend::Memory(Arc::clone(&memory)));

    assert!(worker.request_paste(LayerKind::Overlay));

    assert_eq!(ClipboardPass::wait_for_paste(&worker, LayerKind::Overlay).as_deref(), Some("hello"));
    assert_eq!(worker.take_paste(LayerKind::Hud), None);
}

#[test]
fn a_paste_over_64_kib_is_cut_on_a_char_boundary() {
    // one byte, then two-byte chars, so byte 64 KiB falls inside a char
    let text = format!("a{}", "\u{e9}".repeat(ClipboardWorker::MAX_PASTE_BYTES));
    let memory = Arc::new(Mutex::new(text.clone()));
    let worker = ClipboardWorker::new(ClipboardBackend::Memory(memory));

    assert!(worker.request_paste(LayerKind::Hud));
    let pasted = ClipboardPass::wait_for_paste(&worker, LayerKind::Hud).unwrap_or_default();

    assert_eq!(pasted.len(), ClipboardWorker::MAX_PASTE_BYTES - 1);
    assert!(text.starts_with(&pasted));
}

#[test]
fn a_copy_over_1_mib_is_refused() {
    let memory = Arc::new(Mutex::new("old".to_owned()));
    let worker = ClipboardWorker::new(ClipboardBackend::Memory(Arc::clone(&memory)));

    assert!(!worker.copy("x".repeat(ClipboardWorker::MAX_COPY_BYTES + 1)));
    assert!(worker.copy("new".to_owned()));

    assert!(ClipboardPass::wait_for_text(&memory, "new"));
}

#[test]
fn a_full_queue_refuses_the_request_and_does_not_block() {
    let memory = Arc::new(Mutex::new(String::new()));
    let worker = ClipboardWorker::new(ClipboardBackend::Blocking(Arc::clone(&memory)));
    let busy = memory.lock().unwrap_or_else(|error| panic!("memory clipboard: {error}"));
    // the worker takes this request and waits inside it until the lock is let go
    assert!(worker.copy("in flight".to_owned()));
    assert!(ClipboardPass::wait_for_empty_queue(&worker));

    let accepted = (0..ClipboardWorker::QUEUE).filter(|_| worker.copy("queued".to_owned())).count();
    // the worker cannot take a request while the lock is held, so this returns only if the copy does not wait
    let fifth = worker.copy("refused".to_owned());

    assert_eq!(accepted, ClipboardWorker::QUEUE);
    assert!(!fifth);
    assert_eq!(worker.pending(), ClipboardWorker::QUEUE);
    drop(busy);
}

#[test]
fn a_paste_result_that_is_not_taken_keeps_its_slot_and_the_next_is_dropped() {
    let memory = Arc::new(Mutex::new("first".to_owned()));
    let worker = ClipboardWorker::new(ClipboardBackend::Memory(Arc::clone(&memory)));
    assert!(worker.request_paste(LayerKind::Overlay));
    assert!(worker.copy("second".to_owned()));
    assert!(worker.request_paste(LayerKind::Overlay));
    // requests run in order, so once this copy lands the second paste was read
    assert!(worker.copy("done".to_owned()));
    assert!(ClipboardPass::wait_for_text(&memory, "done"));

    assert_eq!(worker.take_paste(LayerKind::Overlay).as_deref(), Some("first"));
    assert_eq!(worker.take_paste(LayerKind::Overlay), None);
}
