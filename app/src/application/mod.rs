//! Application layer: the coordinator and its event plumbing.

pub mod coordinator;
pub mod event_sink;

pub use coordinator::Coordinator;
pub use event_sink::ChannelSink;
