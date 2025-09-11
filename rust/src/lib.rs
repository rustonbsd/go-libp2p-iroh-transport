mod actor;
mod ffi;
mod node;
mod state;
mod stream;
mod transport;

use std::sync::atomic::AtomicU64;

pub use ffi::{IrohNodeHandle, IrohStreamHandle};
pub use node::IrohNode;
pub use stream::{ConnectionShould, IrohStream};
pub use transport::IrohTransport;

use once_cell::sync::{Lazy, OnceCell};
use tokio::{
    runtime::{Builder, Runtime},
    sync::Mutex,
};

use crate::state::State;

pub const ALPN: &[u8] = b"/libp2p/iroh/0.1.0";

pub static STATE: Lazy<State> = Lazy::new(|| State::new());

static NEXT_TRANSPORT_HANDLE: AtomicU64 = AtomicU64::new(0);
static NEXT_NODE_HANDLE: AtomicU64 = AtomicU64::new(0);
static NEXT_STREAM_HANDLE: AtomicU64 = AtomicU64::new(0);

static RUNTIME: OnceCell<Mutex<Option<Runtime>>> = OnceCell::new();
static RUNTIME_HANDLE: OnceCell<tokio::runtime::Handle> = OnceCell::new();

pub fn runtime_handle() -> anyhow::Result<tokio::runtime::Handle> {
    if RUNTIME.get().is_none() {
        RUNTIME.get_or_init(|| {
            tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
                .init();

            let runtime = Builder::new_multi_thread()
                .enable_all()
                .thread_name("iroh-rt")
                .build()
                .expect("build runtime");
            RUNTIME_HANDLE
                .set(runtime.handle().clone())
                .expect("set runtime handle");
            Mutex::new(Some(runtime))
        });
    }
    println!("runtime_handle called {}", RUNTIME_HANDLE.get().is_some());
    Ok(RUNTIME_HANDLE
        .get()
        .ok_or_else(|| anyhow::anyhow!("runtime not initialized"))?
        .clone())
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

pub fn get_next_transport_handle() -> u64 {
    NEXT_TRANSPORT_HANDLE.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}
