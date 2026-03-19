pub mod server;

pub mod protocol {
    include!(concat!(env!("OUT_DIR"), "/network_protocol.rs"));
}

pub use protocol::TrackpadMessage;
pub use protocol::trackpad_message::{ActionType, PhaseType};
