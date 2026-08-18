use tokio::sync::broadcast::{self, Receiver, Sender};
use tracing::debug;

use crate::event::HadesEvent;

const DEFAULT_EVENT_BUFFER_CAPACITY: usize = 256;

/// Decoupled publish-subscribe event bus for Hades.
#[derive(Debug, Clone)]
pub struct EventBus {
    sender: Sender<HadesEvent>,
}

impl EventBus {
    /// Creates a new `EventBus` with default channel capacity.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_EVENT_BUFFER_CAPACITY)
    }

    /// Creates a new `EventBus` with a specified capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Publishes an event to all active subscribers.
    /// Returns the number of active subscribers who received the event.
    pub fn publish(&self, event: HadesEvent) -> usize {
        debug!(event = ?event, "Publishing event");
        self.sender.send(event).unwrap_or_default()
    }

    /// Subscribes to events emitted on this bus.
    pub fn subscribe(&self) -> Receiver<HadesEvent> {
        self.sender.subscribe()
    }

    /// Returns the number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
