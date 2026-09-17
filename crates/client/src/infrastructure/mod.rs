//! Infrastructure adapters for the client.

pub mod transport;
pub mod ws_transport;

pub use transport::{MockTransport, Transport, TransportError, TransportHandle};
pub use ws_transport::WsTransport;