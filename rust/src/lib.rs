mod ffi;
mod actor;
mod node;
mod transport;
mod stream;

use std::sync::atomic::AtomicU64;

pub use ffi::{IrohNodeHandle, IrohStreamHandle};
pub use node::IrohNode;
pub use transport::IrohTransport;
pub use stream::{ConnectionShould, IrohStream};

use once_cell::sync::Lazy;
use tokio::runtime::{Builder, Runtime};

pub const ALPN: &[u8] = b"/libp2p/iroh/0.1.0";

pub static TRANSPORT: once_cell::sync::OnceCell<IrohTransport> = once_cell::sync::OnceCell::new();

static NEXT_NODE_HANDLE: AtomicU64 = AtomicU64::new(0);
static NEXT_STREAM_HANDLE: AtomicU64 = AtomicU64::new(0);

static RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    Builder::new_multi_thread()
        .enable_all()
        .thread_name("iroh-rt")
        .build()
        .expect("build runtime")
});

pub fn runtime_handle() -> tokio::runtime::Handle {
    RUNTIME.handle().clone()
}

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