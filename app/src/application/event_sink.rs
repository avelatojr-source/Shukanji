//! Bridges background tasks to the egui UI thread.

use std::sync::Arc;

use tokio::sync::mpsc;

use crate::domain::events::{AppEvent, EventSink};

/// A bounded event channel plus a wake callback (usually
/// `egui::Context::request_repaint`). `emit` is non-blocking: if the channel
/// is full or closed the event is dropped with a warning rather than blocking
/// the background task.
pub struct ChannelSink {
    tx: mpsc::Sender<AppEvent>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl ChannelSink {
    pub fn new(tx: mpsc::Sender<AppEvent>, wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self { tx, wake }
    }
}

impl EventSink for ChannelSink {
    fn emit(&self, event: AppEvent) {
        match self.tx.try_send(event) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                tracing::warn!("UI event channel is full; an event was dropped");
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {}
        }
        (self.wake)();
    }
}
