use std::collections::VecDeque;

use crate::structs::toast::{Toast, ToastKind};

/// Holds at most `capacity` toasts. A push into a full queue drops the oldest toast. `Denied` and
/// `Blocked` toasts are limited to one per `rate_ms` for the same (capability, mod). The limit looks
/// at the toasts still held, so a toast pushed out of a full queue no longer limits.
pub struct ToastQueue {
    toasts: VecDeque<Toast>,
    capacity: usize,
    rate_ms: u64,
}

impl ToastQueue {
    pub fn new(capacity: usize, rate_ms: u64) -> Self {
        let capacity = capacity.max(1);
        Self { toasts: VecDeque::with_capacity(capacity), capacity, rate_ms }
    }

    /// Returns false when the toast was refused by the rate limit.
    pub fn push(&mut self, kind: ToastKind, now_ms: u64) -> bool {
        if self.is_rate_limited(&kind, now_ms) {
            return false;
        }
        if self.toasts.len() == self.capacity {
            self.toasts.pop_front();
        }
        self.toasts.push_back(Toast { kind, created_ms: now_ms });
        true
    }

    /// The toasts younger than `ttl_ms`, oldest first.
    pub fn visible(&self, now_ms: u64, ttl_ms: u64) -> impl Iterator<Item = &Toast> {
        self.toasts.iter().filter(move |toast| now_ms.saturating_sub(toast.created_ms) < ttl_ms)
    }

    pub fn len(&self) -> usize {
        self.toasts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.toasts.is_empty()
    }

    fn is_rate_limited(&self, kind: &ToastKind, now_ms: u64) -> bool {
        let Some(key) = kind.rate_key() else {
            return false;
        };
        self.toasts
            .iter()
            .any(|toast| toast.kind.rate_key() == Some(key) && now_ms.saturating_sub(toast.created_ms) < self.rate_ms)
    }
}
