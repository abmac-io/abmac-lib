use std::sync::mpsc;

use crate::Spout;

#[derive(Debug, Clone)]
pub struct ChannelSpout<T> {
    sender: mpsc::Sender<T>,
}

impl<T> ChannelSpout<T> {
    /// Create a new channel spout from a sender.
    pub fn new(sender: mpsc::Sender<T>) -> Self {
        Self { sender }
    }

    /// Get a reference to the underlying sender.
    pub fn sender(&self) -> &mpsc::Sender<T> {
        &self.sender
    }

    /// Consume the spout and return the sender.
    pub fn into_sender(self) -> mpsc::Sender<T> {
        self.sender
    }
}

impl<T> Spout<T> for ChannelSpout<T> {
    #[inline]
    fn send(&mut self, item: T) {
        // Ignore send errors - receiver may have been dropped
        let _ = self.sender.send(item);
    }
}

/// Thread-safe spout wrapper using `Arc<Mutex<S>>`.
///
/// Allows multiple producers to share a single spout with mutex synchronization.
/// Useful for MPSC patterns where all items should go to one collector.
impl<T, S: Spout<T>> Spout<T> for std::sync::Arc<std::sync::Mutex<S>> {
    #[inline]
    fn send(&mut self, item: T) {
        self.lock().unwrap().send(item);
    }

    #[inline]
    fn flush(&mut self) {
        self.lock().unwrap().flush();
    }
}
