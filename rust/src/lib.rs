mod actor;
mod ffi;
mod node;
mod runtime;
mod state;
mod stream;
mod transport;

use std::sync::atomic::AtomicU64;
use once_cell::sync::Lazy;

pub use ffi::{IrohNodeHandle, IrohStreamHandle};
pub use node::IrohNode;
pub use stream::{ConnectionShould, IrohStream};
pub use transport::IrohTransport;

use crate::state::State;

pub const ALPN: &[u8] = b"/libp2p/iroh/0.1.0";

pub static STATE: Lazy<State> = Lazy::new(|| State::new());

static NEXT_TRANSPORT_HANDLE: AtomicU64 = AtomicU64::new(0);
static NEXT_NODE_HANDLE: AtomicU64 = AtomicU64::new(0);
static NEXT_STREAM_HANDLE: AtomicU64 = AtomicU64::new(0);

pub fn peer_id_to_ed25519_public_key(peer_id: &str) -> Option<iroh::NodeId> {
    let decoded = bs58::decode(peer_id.as_bytes()).into_vec().ok()?;
    if decoded.len() != 38 {
        return None;
    }
    iroh::NodeId::from_bytes(&decoded[6..].try_into().ok()?).ok()
}

pub fn get_next_node_handle() -> u64 {
    NEXT_NODE_HANDLE.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}

pub fn get_next_stream_handle() -> u64 {
    NEXT_STREAM_HANDLE.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}

pub fn get_next_transport_handle() -> u64 {
    NEXT_TRANSPORT_HANDLE.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}
